# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

import subprocess


def _retired_sidecar_gate_ttl_source_errors(
    path: Path,
    source: str,
    role: str,
) -> list[str]:
    """Reject any server-request gate TTL identifier in executable Rust."""

    retired_ttl_tokens = sorted(
        {
            token
            for token in rust_code_tokens(source)
            if "ttl" in token.lower()
            and all(
                fragment in token.lower()
                for fragment in ("server", "request", "gate")
            )
        }
    )
    if not retired_ttl_tokens:
        return []
    return [
        f"{path}: retired wall-clock sidecar gate TTL must remain absent "
        f"from production; found identifiers {retired_ttl_tokens} in the "
        f"{role} seam"
    ]

def _require_leader_wire_height_ingress_retirement(
    height_ingress_path: Path,
    ingress_path: Path,
    ingress_source: str,
    leader_wire_store_path: Path,
    leader_wire_store_source: str,
    errors: list[str],
) -> None:
    """Bind atomic ingress retirement and its exact durable carrier handoff."""

    height_ingress_source = height_ingress_path.read_text(encoding="utf-8")
    # Production retirement is owned by queue-level close and the atomic
    # leader-wire lifecycle retirement. Retired Certified-Serve and joint
    # height-ingress gate wrappers are not first-release seams.
    height_ingress_binding_items: dict[str, RustItem | None] = {
        "runner::close_ingress_for_rollover": (
            _require_rust_item(
                height_ingress_path,
                height_ingress_source,
                "close_ingress_for_rollover",
                errors,
            )
        )
    }
    for item_name in (
        "retire_leader_wire_lifecycle_gate",
        "close",
    ):
        height_ingress_binding_items[f"ingress::{item_name}"] = (
            _require_qualified_rust_item(
                ingress_path,
                ingress_source,
                "FairV2Ingress",
                item_name,
                errors,
                f"leader-wire height-ingress transaction FairV2Ingress::{item_name}",
            )
        )
    height_ingress_binding_items["leader_wire_store::park_sealed_ingress"] = (
        _require_qualified_rust_item(
            leader_wire_store_path,
            leader_wire_store_source,
            "LeaderWireLifecycleStoreGate",
            "park_sealed_ingress",
            errors,
            "leader-wire durable retirement LeaderWireLifecycleStoreGate::park_sealed_ingress",
        )
    )
    expected_height_ingress_binding_keys = {
        "runner::close_ingress_for_rollover",
        "ingress::retire_leader_wire_lifecycle_gate",
        "leader_wire_store::park_sealed_ingress",
        "ingress::close",
    }
    observed_height_ingress_binding_keys = set(
        _PRODUCTION_HEIGHT_INGRESS_BINDING_ITEM_SHA256
    )
    if observed_height_ingress_binding_keys != expected_height_ingress_binding_keys:
        errors.append(
            f"{height_ingress_path}: leader-wire height-ingress token-seal inventory mismatch: "
            f"missing={sorted(expected_height_ingress_binding_keys - observed_height_ingress_binding_keys)}, "
            f"extra={sorted(observed_height_ingress_binding_keys - expected_height_ingress_binding_keys)}"
        )
    for qualified_name, expected_sha256 in (
        _PRODUCTION_HEIGHT_INGRESS_BINDING_ITEM_SHA256.items()
    ):
        if qualified_name.startswith("ingress::"):
            path = ingress_path
        elif qualified_name.startswith("leader_wire_store::"):
            path = leader_wire_store_path
        else:
            path = height_ingress_path
        _require_rust_item_token_sha256(
            path,
            height_ingress_binding_items.get(qualified_name),
            expected_sha256,
            f"leader-wire height-ingress ownership {qualified_name}",
            errors,
        )

    _require_exact_rust_tokens(
        height_ingress_path,
        height_ingress_binding_items["runner::close_ingress_for_rollover"],
        """
fn close_ingress_for_rollover(ingress_ready: &AtomicBool, block_ingress: &FairV2Ingress) {
    ingress_ready.store(false, Ordering::Release);
    block_ingress.close();
}
""",
        "rollover close must publish not-ready before closing fair-ingress admission",
        errors,
    )
    _require_exact_rust_tokens(
        ingress_path,
        height_ingress_binding_items["ingress::close"],
        """
pub(crate) fn close(&self) {
    self.state.lock().open = false;
}
""",
        "fair ingress close must make admission unavailable under the queue lock",
        errors,
    )
    atomic_retirement_item = height_ingress_binding_items[
        "ingress::retire_leader_wire_lifecycle_gate"
    ]
    atomic_retirement_description = (
        "atomic leader-wire retirement must exclude consumers and producers, "
        "park the exact carrier set durably, and clear volatile ownership only afterward"
    )
    for expected_source in (
        """
let _service_guard = self.service_lock.lock();
let mut state = self.state.lock();
state.open = false;
let bound = state
    .leader_wire_lifecycle_gate
    .as_ref()
    .cloned()
    .ok_or_else(|| "leader-wire lifecycle gate was already unbound".to_owned())?;
if !serviced_candidate_store::LeaderWireLifecycleStoreGate::ptr_eq(&bound, gate) {
    return Err("leader-wire lifecycle gate changed per-height ownership".to_owned());
}
""",
        """
if !inbound_ownership.validate_exact()
    || !entry.ownership_snapshot.validate_exact()
    || entry.leader_wire_token.as_ref() != inbound_ownership.leader_wire_token()
    || entry.leader_wire_token.as_ref() != entry.ownership_snapshot.leader_wire_token()
{
    return Err(
        "sealed leader-wire ingress changed a queued ownership projection".to_owned(),
    );
}
""",
        """
let Some(token) = entry.leader_wire_token.as_ref() else {
    continue;
};
if token.identity.context_id != context_id
    || token.identity.height != height
    || carriers.insert(token.slot.clone(), token.clone()).is_some()
{
    return Err(
        "sealed leader-wire ingress changed its exact retiring carrier set".to_owned(),
    );
}
""",
        """
let mirrored_ingress = state
    .leader_wire_lifecycles
    .iter()
    .filter_map(|(slot, record)| {
        (record.status == FairV2IngressLeaderWireStatus::Ingress)
            .then(|| (slot.clone(), record.token.clone()))
    })
    .collect::<BTreeMap<_, _>>();
if carriers != mirrored_ingress {
    return Err(
        "sealed leader-wire ingress disagreed with live carrier ownership".to_owned(),
    );
}
let retirement = bound.park_sealed_ingress(carriers)?;
""",
        """
let retirement = bound.park_sealed_ingress(carriers)?;
let empty_lanes = state
    .roster
    .iter()
    .cloned()
    .map(|peer| {
        (
            FairV2IngressSource::Validator(peer),
            FairV2IngressLane::default(),
        )
    })
    .collect::<BTreeMap<_, _>>();
state.lanes = empty_lanes;
""",
        """
state.lanes = empty_lanes;
state.pending_wire_owners.clear();
state.ready.clear();
state.len = 0;
state.bytes = 0;
state.nonempty_since = None;
state.last_service_attempt_at = None;
state.leader_wire_lifecycles.clear();
state.leader_wire_lifecycle_gate = None;
state.leader_wire_lifecycle_ordinals = None;
state.leader_wire_context = None;
self.debug_assert_consistent(&state);
retirement.complete();
Ok(())
""",
    ):
        _require_rust_token_sequence(
            ingress_path,
            atomic_retirement_item,
            expected_source,
            atomic_retirement_description,
            errors,
        )

    durable_retirement_item = height_ingress_binding_items[
        "leader_wire_store::park_sealed_ingress"
    ]
    durable_retirement_description = (
        "durable leader-wire retirement must validate the exact Ingress set, "
        "roll back failed persistence, and mint its receipt only after fsync"
    )
    for expected_source in (
        """
if carriers.iter().any(|(slot, token)| {
    slot != &token.slot
        || !token.validate_exact(
            self.context_id,
            self.height,
            &self.roster,
            self.max_chunk_count,
        )
}) {
    return Err("sealed leader-wire ingress changed immutable geometry".to_owned());
}
""",
        """
if durable_ingress != carriers
    || carriers
        .keys()
        .any(|slot| state.replay_dormant.contains(slot))
    || carriers.iter().any(|(slot, token)| {
        state.records.get(slot).is_none_or(|record| {
            record.token != *token
                || record.status != LeaderWireLifecycleStatus::Ingress
                || record.runtime_owner.is_some()
                || record.terminal_evidence.is_some()
        })
    })
{
    return Err(
        "sealed leader-wire ingress disagreed with durable carrier ownership".to_owned(),
    );
}
""",
        """
record.status = LeaderWireLifecycleStatus::Dormant;
let inserted = state.replay_dormant.insert(slot.clone());
debug_assert!(inserted);
""",
        """
if !carriers.is_empty()
    && let Err(error) = self.persist_locked(&state)
{
    *state = previous;
    return Err(error);
}
Ok(SealedLeaderWireIngressRetirementV1 { _private: () })
""",
    ):
        _require_rust_token_sequence(
            leader_wire_store_path,
            durable_retirement_item,
            expected_source,
            durable_retirement_description,
            errors,
        )


def _require_exact_output_startup_and_successor_rollover_seams(
    lane_path: Path,
    lane_ack_items: dict[str, RustItem | None],
    runner_path: Path,
    runner_startup_item: RustItem | None,
    runner_ack_items: dict[str, RustItem | None],
    lifecycle_runner_path: Path, lifecycle_runner_items: dict[str, RustItem | None],
    pending_runner_path: Path, errors: list[str],
) -> None:
    """Check exact startup activation and successor rollover ownership seams."""

    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get(
            "V2LaneWorkAdapter::new_with_output_guard_and_transport_inner"
        ),
        "adapter.hydrate_canonical_lane_artifacts()?;",
        "the production constructor must remain carrier-silent before exact Queue installation",
        errors,
        count=0,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get(
            "V2LaneWorkAdapter::new_with_output_guard_and_transport_inner"
        ),
        "adapter.drive_lane_sessions();",
        "the production constructor must not drive a lane session before exact Queue installation",
        errors,
        count=0,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get(
            "V2LaneWorkAdapter::activate_after_lane_drain_queue_install"
        ),
        """
if !Arc::ptr_eq(installed_queue, queue) {
    return Err(V2LaneWorkError::InvalidContext(
        "lane-work startup activation names a different queue source".to_owned(),
    ));
}
let installed_queue = Arc::clone(installed_queue);
let output_guard = Arc::clone(&self.output_guard);
let activation = output_guard
    .begin_fail_stop_operation()
    .ok_or(V2LaneWorkError::RestartRequired)?;
self.hydrate_canonical_lane_artifacts()?;
self.revalidate_hydrated_autonomous_queue_owners(installed_queue.as_ref())?;
self.startup_activation_complete = true;
self.drive_lane_sessions();
activation.complete();
Ok(())
""",
        "one-shot startup activation must authenticate the installed Queue, fail-stop hydration, revalidate every local reservation owner, and only then drive lane sessions",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        runner_startup_item,
        """
let _initial_local_validator =
    local_validator_index(verified_context.context(), &local_peer, config.role)?;
let _lifecycle_process_generation = claim_runner_lifecycle_process_generation(
    config.role,
    kura.as_ref(),
    verified_context.context(),
    &local_peer,
)?;
""",
        "runner startup must classify height-local duty before acquiring one configured-role process generation",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        runner_ack_items.get("claim_runner_lifecycle_process_generation"),
        """
match role {
    NodeRole::Observer => Ok(None),
    NodeRole::Validator => kura
        .claim_autonomous_lifecycle_process_generation(
            context.network_id,
            local_peer
        )
        .map(Some)
        .map_err(|error| {
            V2RunnerError::Service(format!(
                "failed to claim the durable autonomous lifecycle process generation: {error}"
            ))
        }),
}
""",
        "the configured-role process generation helper must durably claim validator ownership by passing the authenticated context NetworkId directly with the local identity",
        errors,
    )
    for forbidden_identity_bypass_token in (
        "context.roster",
        "local_validator_index",
        "_initial_local_validator",
        "context.chain_id",
        "lifecycle_chain_id",
        "Hash::new",
        "NetworkId::default",
        "Default::default",
        "synthetic_network_id",
    ):
        _require_rust_token_sequence(
            runner_path,
            runner_ack_items.get("claim_runner_lifecycle_process_generation"),
            forbidden_identity_bypass_token,
            "the configured-role process generation helper must not consult height-local roster membership or derive/substitute a legacy/default/foreign NetworkId",
            errors,
            count=0,
        )
    _require_rust_token_sequence(
        lifecycle_runner_path,
        lifecycle_runner_items.get("ordinary_loop"),
        """
reconcile_autonomous_lifecycle_startup(
    state.as_ref(),
    queue.as_ref(),
    kura.as_ref(),
    &context,
    planner_evidence,
    deferred_terminal_recovery,
    lifecycle_process_generation.as_ref(),
""",
        "startup reconciliation must borrow the exact process-lifetime generation claim",
        errors,
    )
    _require_rust_token_sequence(
        pending_runner_path,
        lifecycle_runner_items.get("pending_loop"),
        """reconcile_pending_lane_startup(
pending, &mut setup_runner, &mut activation, &context, &verified_context, &state,
&queue, &kura, &local_peer, &common_config.key_pair, &output_guard,
lane_work_limits, retransmit_interval, control_queue_capacity, &wake_rx,
&shutdown_signal, lifecycle_process_generation.as_ref(),
)?""",
        "pending-Kura startup reconciliation must borrow the exact process-lifetime generation claim",
        errors,
    )
    for path, item in (
        (lifecycle_runner_path, lifecycle_runner_items.get("ordinary_loop")),
        (pending_runner_path, lifecycle_runner_items.get("pending_loop"))):
        _require_rust_token_sequence(
            path,
            item,
            """V2LaneWorkAdapter::new_with_output_guard_and_transport(
&verified_context, local_peer.clone(), common_config.key_pair.clone(),
config.role == NodeRole::Validator,
""",
            "each lane adapter must be constructed from the same configured role after startup reconciliation",
            errors,
        )
        _require_rust_token_sequence(
            path,
            item,
            "lifecycle_process_generation.clone(),",
            "each lane adapter must receive the same process-lifetime generation claim",
            errors,
        )
    _require_rust_token_sequence(
        lifecycle_runner_path,
        lifecycle_runner_items.get("ordinary_loop"),
        """
lane_work.install_lane_drain_queue(Arc::clone(&queue))?;
lane_work.activate_after_lane_drain_queue_install(&queue)?;
""",
        "runner startup must install the exact Queue before the one-shot carrier activation seam",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get("RetainedMergeSidecars::rehydrate_for_successor"),
        """
if self.successor_context_id != successor.id()
    || self.successor_context_hash != HashOf::new(successor)
{
    return Err(V2LaneWorkError::InvalidContext(
        "retained merge-sidecar handoff names another successor context".to_owned(),
    ));
}
let authority = DurableMergeSidecarRolloverAuthority {
    _exact_output_handoff: self.exact_output_handoff,
};
self.transport
    .rehydrate_with_exact_geometry_after_durable_handoff(
        reply_source_capacity,
        limits,
        server_stream_capacity,
        server_roster_digest,
        now,
        authority,
    )
""",
        "retained sidecar ownership must bind the exact successor context and consume its durable output handoff before roster-aware rehydration",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get("V2LaneWorkAdapter::into_retained_merge_sidecars"),
        """
if self.has_pending_committed_output_handoff() {
    return Err(V2LaneWorkError::InvalidContext(
        "retained merge-sidecar handoff still owns committed lane output".to_owned(),
    ));
}
if self.effect_count() != 0 {
    return Err(V2LaneWorkError::InvalidContext(
        "retained merge-sidecar handoff still owns undispatched lane output".to_owned(),
    ));
}
successor
    .validate()
    .map_err(|error| V2LaneWorkError::InvalidContext(error.to_string()))?;
""",
        "lane rollover must prove all committed and queued output empty before validating the immediate successor",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get("V2LaneWorkAdapter::into_retained_merge_sidecars"),
        """
if !exact_output_handoff
    .is_bound_to_transport_owner(&self.exact_output_handoff_owner)
{
    return Err(V2LaneWorkError::InvalidContext(
        "durable exact-output handoff belongs to another service/transport owner".to_owned(),
    ));
}
if !exact_output_handoff.matches_predecessor_context(&self.context)
    || !exact_output_handoff.matches_finality_artifact(artifact)
{
    return Err(V2LaneWorkError::InvalidContext(
        "durable exact-output handoff belongs to another predecessor artifact".to_owned(),
    ));
}
if !exact_output_handoff.authorizes_immediate_successor(successor) {
    return Err(V2LaneWorkError::InvalidContext(
        "durable exact-output handoff does not authorize the immediate successor".to_owned(),
    ));
}
""",
        "lane rollover must consume only its paired service receipt for the exact predecessor artifact and immediate successor",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get("V2LaneWorkAdapter::into_retained_merge_sidecars"),
        """
let retained = RetainedMergeSidecars {
    transport: self.merge_sidecars,
    exact_output_handoff,
    successor_context_id: successor.id(),
    successor_context_hash: HashOf::new(successor),
};
handoff.complete();
Ok(retained)
""",
        "lane rollover must bind retained transport ownership to the canonical successor before completing the fail-stop operation",
        errors,
    )

_KURA_PRODUCTION_COMPONENT_FILES = (
    "kura/startup_finality_support.rs",
    "kura/bound_progress_and_retained_support.rs",
    "kura/autonomous_reservation_bounds.rs",
    "kura/certified_bundle_capacity_reservation_types.rs",
    "kura/prune_commit_merge_support.rs",
    "kura/merge_ledger_latest_execution_index.rs",
    "kura/replica_advert_and_body_status.rs",
    "kura/retired_pipeline_roster_rejection.rs",
    "kura/retained_finality_replica_authority.rs",
    "kura/queue_plan_admission_batch.rs",
    "kura/wsv_checkpoint_read_helpers.rs",
    "kura/durable_block_and_atomic_sidecar_io.rs",
    "kura/prune_intent_publication.rs",
    "kura/prune_recovery_capacity.rs",
    "kura/block_store_definition_and_test_controls.rs",
    "kura/startup_finality_session_reads.rs",
    "kura/pipeline_and_lane_artifacts.rs",
    "kura/autonomous_terminal_capacity.rs",
    "kura/autonomous_publication_temp_recovery.rs",
    "kura/historical_autonomous_recovery_temp_reconciliation.rs",
    "kura/hot_path_capacity_preflight.rs",
    "kura/autonomous_execution_view_capacity.rs",
    "kura/certified_bundle_capacity.rs",
    "kura/lane_artifact_budget.rs",
    "kura/autonomous_lifecycle_terminal_outcomes.rs",
    "kura/autonomous_release_authority.rs",
    "kura/autonomous_retired_attempt.rs",
    "kura/autonomous_application_evidence.rs",
    "kura/indexed_sidecar_io.rs",
    "kura/indexed_sidecar_rewrite.rs",
    "kura/lane_history_compaction.rs",
    "kura/prune_block_store_tail.rs",
    "kura/test_fault_injection_state.rs",
    "kura/test_fault_injection_controls.rs",
    "kura/file_error_support.rs",
)

_REVIEWED_RUST_INCLUDE_MANIFESTS = {
    'crates/iroha_core/src/block.rs': (
        'block/autonomous_merge_carrier_content_tests.rs',
        'block/exact_quorum_cardinality_tests.rs',
        'block/autonomous_anchor_network_tests.rs',
        'block/sccp_soracloud_validation_tests.rs',
        'block/canonical_genesis_validation_tests.rs',
        'block/genesis_validation_regression_tests.rs',
        'block/axt_shared_budget_across_envelopes_test.rs',
        'block/scheduler_variant_tests.rs',
        'block/validation_native_amx_test_support.rs',
        'block/native_amx_receipt_regression_tests.rs',
        'block/native_amx_exact_quorum_cardinality_tests.rs',
        'block/native_amx_and_dag_tests.rs',
        'block/sequential_rejected_pipeline_trigger_tests.rs',
        'block/tx_order_validation_revalidation_test.rs',
        'block/rejected_live_batch_fee_tests.rs',
        'block/fee_admission_tests.rs',
        'block/bootstrap_and_genesis_tests.rs',
    ),
    'crates/iroha_config/src/parameters/actual.rs': (
        'actual/torii_tx_history.rs',
        'actual/torii_http_transport.rs',
        'actual/torii_mcp_profile.rs',
        'actual/offline.rs',
        'actual/tests.rs',
    ),
    'crates/iroha_config/src/parameters/actual/tests.rs': (
        'sora_profile_discovery_disabled_test.rs',
        'sora_profile_runtime_tests.rs',
    ),
    'crates/iroha_config/src/parameters/user.rs': (
        'user/kura.rs',
        'user_soranet_handshake_tests.rs',
        'user/torii_peer_geo.rs',
        'user/torii_soranet_privacy_ingest.rs',
        'user/torii_tx_history.rs',
        'user/sorafs_moderation_query_bound_tests.rs',
        'user/governance_dag_head_mode_tests.rs',
        'user/zk_prover_report_retention_tests.rs',
        'user/query_fanout_memory_tests.rs',
        'user/app_routed_read_body_timeout_tests.rs',
        'user/operator_signature_body_timeout_tests.rs',
        'user/verified_source_ingress_tests.rs',
        'user/iso_bridge_store_memory_tests.rs',
        'user/kura_and_snapshot_tests.rs',
        'user/runtime_tail_tests.rs',
    ),
    'crates/iroha_data_model/src/block/consensus_v2.rs': (
        'consensus_v2_tests.rs',
    ),
    'crates/iroha_data_model/src/block/consensus_v2_tests.rs': (
        'consensus_v2_json_tests.rs',
    ),
    'crates/iroha_core/src/kura.rs': (
        *_KURA_PRODUCTION_COMPONENT_FILES,
        'kura/tests/00_bounded_sidecar_read_tests.rs',
        'kura/tests/01_support_snapshot_bootstrap_and_rewrite.rs',
        'kura/tests/01_prune_capacity_support.rs',
        'kura/tests/01a_retained_eviction_and_rewrite_tail.rs',
        'kura/tests/02_replacement_and_preflight.rs',
        'kura/tests/02a_fresh_single_lane_preflight.rs',
        'kura/tests/03_preflight_and_merge_entry.rs',
        'kura/tests/03a_preflight_and_merge_entry_tail.rs',
        'kura/tests/04_merge_log_and_associations.rs',
        'kura/tests/04b_merge_artifact_budget.rs',
        'kura/tests/04c_canonical_association_capacity.rs',
        'kura/tests/04d_prune_intent_capacity.rs',
        'kura/tests/05_merge_resolution_and_eviction.rs',
        'kura/tests/05a_replica_advert_and_body_eviction.rs',
        'kura/tests/06_eviction_and_autonomous_lanes.rs',
        'kura/tests/07a_autonomous_reservation_reconciliation_support.rs',
        'kura/tests/07_autonomous_lanes_and_sidecars.rs',
        'kura/tests/07b_autonomous_reservation_reconciliation_tests.rs',
        'kura/tests/07c_lane_execution_sidecar_tests.rs',
        'kura/tests/07d_strict_lane_ownership_barrier_tests.rs',
        'kura/tests/07e_autonomous_lifecycle_and_canonical_artifact_tests.rs',
        'kura/tests/07e_autonomous_publication_temp_recovery_tests.rs',
        'kura/tests/07e_terminal_capacity_hardening_tests.rs',
        'kura/tests/07f_canonical_carrier_terminal_recovery_tests.rs',
        'kura/tests/07g_claim_capacity_preflight_tests.rs',
        'kura/tests/07h_autonomous_execution_view_capacity_tests.rs',
        'kura/tests/07i_historical_autonomous_batch_capacity_tests.rs',
        'kura/tests/07j_certified_bundle_capacity_tests.rs',
        'kura/tests/07k_historical_atomic_temp_recovery_tests.rs',
        'kura/tests/07l_pending_canonical_capacity_tests.rs',
        'kura/tests/08_lane_receipts_and_artifacts.rs',
        'kura/tests/08a_certified_lane_block_read_tests.rs',
        'kura/tests/08b_lane_history_compaction_capacity_tests.rs',
        'kura/tests/09_lane_artifacts_and_fastpq.rs',
        'kura/tests/10_native_amx_and_roster.rs',
        'kura/tests/10b_native_amx_prepublication_transition.rs',
        'kura/tests/11_roster_and_progress_sidecars.rs',
        'kura/tests/12_sidecar_index_and_pruning.rs',
        'kura/tests/13_manifests_and_fsync.rs',
    ),
    'crates/iroha_core/src/kura/autonomous_application_evidence.rs': (
        'passive_diagnostic_reads.rs',
    ),
    'crates/iroha_core/src/kura/tests/10_native_amx_and_roster.rs': (
        '10c_native_amx_latest_index_support_and_bounds.rs',
    ),
    'crates/iroha_core/src/kura/pipeline_and_lane_artifacts.rs': (
        'autonomous_merge_bundle_support.rs',
        'autonomous_reservation_types.rs',
        'autonomous_reservation_inventory.rs',
        'autonomous_reservation_classifier.rs',
        'historical_autonomous_recovery.rs',
        'native_amx_participant_application_artifacts.rs',
    ),
    'crates/iroha_core/src/kura/lane_geometry.rs': (
        'lane_geometry/bootstrap_path_safety.rs',
        'lane_geometry/bootstrap_relabel.rs',
        'lane_geometry/catalog_validation.rs',
        'lane_geometry/retirement_bounds.rs',
        'lane_geometry_tests/00_support.rs',
        'lane_geometry/native_amx_retained_window_tests.rs',
        'lane_geometry_tests/00_retirement.rs',
        'lane_geometry_tests/01_retirement_and_recovery.rs',
        'lane_geometry_tests/02_geometry_moves_and_journal.rs',
        'lane_geometry_tests/03_gc_and_startup.rs',
    ),
    'crates/iroha_core/src/merge_sidecar.rs': (
        'merge_sidecar_signing_guard_tests.rs',
    ),
    'crates/iroha_core/src/queue.rs': (
        'queue/canonical_terminal_cleanup.rs',
        'queue/nexus_reconfigure_manifest_reload_tests.rs',
        'queue/privacy_governance_compliance_tests.rs',
        'queue/plan_journal_startup_atomicity_tests.rs',
        'queue/global_guard_claim_conflict_tests.rs',
        'queue/transaction_guard_return_tests.rs',
        'queue/queue_metadata_and_admission_tests.rs',
        'queue/instruction_and_state_routing_tests.rs',
        'queue/routing_batch_admission_tests.rs',
        'queue/config_factory_test_support.rs',
        'queue/teu_limit_and_backlog_tests.rs',
        'queue/routing_projection_resilience_tests.rs',
        'queue/capacity_and_concurrency_tests.rs',
        'queue/pressure_resync_tests.rs',
        'queue/expiry_tracking_tests.rs',
        'queue/inflight_tracking_tests.rs',
        'queue/lane_reservation_tests.rs',
        'queue/lane_reservation_terminal_fault_tests.rs',
        'queue/reservation_recovery_tests.rs',
    ),
    'crates/iroha_core/src/queue/instruction_and_state_routing_tests.rs': (
        'gossip_routing_metadata_tests.rs',
        'gossip_route_validation_tests.rs',
        'drain_revalidation_tests.rs',
    ),
    'crates/iroha_core/src/queue/lane_reservation_tests.rs': (
        'lane_reservation_core_tests.rs',
    ),
    'crates/iroha_core/src/queue/reservation_recovery_tests.rs': (
        'retired_release_snapshot_recovery_tests.rs',
        'native_amx_reservation_tests.rs',
    ),
    'crates/iroha_core/src/queue/journal.rs': (
        'journal_reservation_commit_preflight.rs',
        'journal_direct_file_io.rs',
        'plan_journal_bounds_tests.rs',
        'plan_journal_replay_tests.rs',
    ),
    'crates/iroha_core/src/smartcontracts/ivm/host.rs': (
        'host/axt_persistent_budget_tests.rs',
        'host/core_codec_and_contract_tests.rs',
        'host/core_query_execution_tests.rs',
        'host/core_query_pagination_tests.rs',
        'host/nested_contract_state_and_rollback_tests.rs',
        'host/zk_verification_tests.rs',
        'host/prepared_public_arguments_tests.rs',
        'host/pointer_abi_validation_tests.rs',
        'host/pointer_abi_and_sm_tests.rs',
    ),
    'crates/iroha_core/src/state.rs': (
        'state/vpn_lease_validation.rs',
        'state/axt_handle_budget.rs',
        'state/zk_asset_state.rs',
        'state/restored_staking_owner_tests.rs',
        'state/confidential_policy_transition_index_tests.rs',
        'state/passive_lane_diagnostic_methods.rs',
        'state/runtime_configuration.rs',
        'state/lane_lifecycle_support.rs',
        'state/diagnostic_state_generation.rs',
        'state/autonomous_predecessor_application.rs',
        'state/state_commit_lock_order_tests.rs',
        'state/transfer_transcript_tests.rs',
        'state/block_proof_tests.rs',
        'state/range_bounds.rs',
        'state/deserialize_core.rs',
        'state/deserialize_world.rs',
        'state/default_oracle.rs',
    ),
    'crates/iroha_core/src/snapshot.rs': (
        'snapshot/support_policy_tests.rs',
        'snapshot/write_roundtrip_tests.rs',
        'snapshot/reconciliation_generation_tests.rs',
    ),
    'crates/iroha_core/src/sumeragi/evidence.rs': (
        'evidence/missing_signer_pop_test.rs',
    ),
    'crates/iroha_core/src/sumeragi/serviced_candidate_store.rs': (
        'serviced_candidate_store_cases.rs',
    ),
    'crates/iroha_core/src/sumeragi/serviced_candidate_store_cases.rs': (
        'serviced_candidate_store/body_terminal_recovery_tests.rs',
        'serviced_candidate_store_tail_tests.rs',
    ),
    'crates/iroha_p2p/src/network.rs': (
        'network/handle_update_tests.rs',
        'network/queue_depth_tests.rs',
    ),
    'crates/iroha_p2p/src/peer.rs': (
        'peer_handshake_config_tests.rs',
        'peer_state_tests.rs',
        'peer_consensus_mode_test.rs',
        'peer_tests.rs',
    ),
    'crates/irohad/src/main.rs': (
        'main/kagemusha_runtime_effective_config_projection.rs',
        'main/shared_sorafs_provider_cache_tests.rs',
        'main/runtime_deps.rs',
        'sumeragi_lane_relay_item.rs',
        'main/online_peers_provider.rs',
        'main/resolved_genesis_trust_anchor_wrong_hash_test.rs',
        'main_tests/governance_dag_publisher_binding_signer.rs',
        'main/governance_dag_launcher_tests.rs',
        'main/kagemusha_runtime_effective_config_projection_tests.rs',
        'main/kagemusha_startup_source_tests.rs',
        'main/runtime_budget_and_config_tests.rs',
        'main/startup_tail_tests.rs',
    ),
    'crates/iroha_core/src/sumeragi/mod.rs': (
        'fair_v2_ingress_leader_wire_identity.rs',
        'fair_v2_ingress_selector.rs',
        'tests/queue_plan_admission_handoff.rs',
        'tests/mod_authoritative_runtime_gate_01_support.rs',
        'tests/mod_authoritative_runtime_gate_02_carrierless_replay.rs',
        'tests/mod_authoritative_runtime_gate_03_admission_and_fairness.rs',
        'tests/mod_authoritative_runtime_gate_04_routes_and_dequeue.rs',
        'tests/mod_authoritative_runtime_gate_05_ownership_maintenance.rs',
        'tests/mod_authoritative_runtime_gate_06_source_isolation.rs',
        'tests/mod_authoritative_runtime_gate_07_wire_bounds.rs',
        'tests/mod_authoritative_runtime_gate_08_capacity_and_control.rs',
        'tests/mod_authoritative_runtime_gate_09_checked_dequeue.rs',
        'tests/mod_authoritative_runtime_gate_09_snapshot_and_source_lanes.rs',
    ),
    'crates/iroha_core/src/sumeragi/status.rs': (
        'status/test_guards.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2.rs': (
        'v2_adapter_persistence_and_wal_types.rs',
        'v2_recovered_decision_validate_adapter_startup.rs',
        'v2_authenticated_recovered_adapter_startup_impl.rs',
        'v2_verified_height_context_recovered_output_auth.rs',
        'v2_adapter_equivocation_evidence.rs',
        'v2_ready_durable_validate_adapter_preview.rs',
        'v2_recovered_lifecycle_sign_completion.rs',
        'v2_wire_registry_and_authentication.rs',
        'tests/v2_adapter_main_00.rs',
        'tests/v2_adapter_main_01.rs',
        'tests/v2_adapter_main_02.rs',
        'tests/v2_adapter_main_03.rs',
        'tests/v2_adapter_main_04.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_recovery.rs': (
        'v2_recovery_tests.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs': (
        'v2_lifecycle_coordinator_state_helpers.rs',
        'tests/v2_lifecycle_coordinator_explorer_cases.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs': (
        'v2_lifecycle_launch_tests.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_launch_tests.rs': (
        'v2_lifecycle_launch_ready_proposal_sign_test_fixtures.rs',
        'v2_lifecycle_launch_recovered_sign_settlement_source_tests.rs',
        'v2_lifecycle_launch_recovered_fetch_source_tests.rs',
        'v2_lifecycle_launch_certified_response_retry_source_tests.rs',
        'v2_lifecycle_launch_recovered_fetch_settlement_source_tests.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs': (
        'v2_lifecycle_ledger_operations.rs',
        'v2_lifecycle_ledger_store.rs',
        'v2_lifecycle_ledger_tests.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests.rs': (
        'v2_lifecycle_ledger_tests_durable_recovery_01.rs',
        'v2_lifecycle_ledger_tests_durable_recovery_02.rs',
        'v2_lifecycle_ledger_tests_frame_and_store.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_projection.rs': (
        'tests/v2_lifecycle_projection_cases.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs': (
        'tests/v2_lifecycle_scheduler_completion_cases.rs',
        'tests/v2_lifecycle_scheduler_certified_serve_cases.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs': (
        'v2_lifecycle_replay_authority_recovered_decision_validate.rs',
        'v2_lifecycle_replay_authority_live_wal.rs',
        'v2_lifecycle_replay_authority_certified_serve.rs',
        'v2_lifecycle_replay_authority_certified_body.rs',
        'v2_lifecycle_replay_authority_payload_projection.rs',
        'v2_lifecycle_replay_authority_output_recovery.rs',
        'tests/v2_lifecycle_replay_authority_fixtures.rs',
        'tests/v2_lifecycle_replay_authority_cases.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery.rs': (
        'v2_lifecycle_work_registry_validate_recovery_registry_impl.rs',
        'v2_lifecycle_work_registry_validate_recovery_parent.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs': (
        'v2_lifecycle_work_registry_validate_recovery_census_impl.rs',
        'v2_lifecycle_work_registry_validate_recovery_registry_tail_impl.rs',
        'v2_lifecycle_work_registry_validate_completion_impl.rs',
        'v2_lifecycle_work_registry_access_impl.rs',
        'v2_lifecycle_work_registry_validate_recovery_execution_impl.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs': (
        'v2_lifecycle_work_registry_body_validate_carriers.rs',
        'v2_lifecycle_work_registry_pre_admission.rs',
        'v2_lifecycle_work_registry_live_wal_sign.rs',
        'v2_lifecycle_work_registry_output.rs',
        'v2_lifecycle_work_registry_live_validate_children.rs',
        'v2_lifecycle_work_registry_recovered_wal.rs',
        'v2_lifecycle_work_registry_validate_recovery.rs',
        'v2_lifecycle_work_registry_validate_execution.rs',
        'v2_lifecycle_work_registry_validate_sidecar.rs',
        'tests/v2_lifecycle_work_registry_00.rs',
        'tests/v2_lifecycle_work_registry_01.rs',
        'tests/v2_lifecycle_work_registry_02.rs',
        'tests/v2_lifecycle_work_registry_validate_dispatch_cases.rs',
        'tests/v2_lifecycle_work_registry_validate_dispatch_execution_cases.rs',
        'tests/v2_lifecycle_work_registry_validate_sidecar_cases.rs',
        'tests/v2_lifecycle_work_registry_durable_store_and_validate_cases.rs',
        'tests/v2_lifecycle_work_registry_exact_registry_cases.rs',
        'tests/v2_lifecycle_work_registry_recovery_surface_cases.rs',
        'tests/v2_lifecycle_work_registry_replay_evidence_cases.rs',
    ),
    'crates/iroha_core/src/sumeragi/tests/v2_lifecycle_work_registry_validate_dispatch_execution_cases.rs': (
        'v2_lifecycle_work_registry_validate_apply_cases.rs',
        'v2_lifecycle_work_registry_validate_completion_cases.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal.rs': (
        'v2_lifecycle_work_registry_recovered_wal_persisted_ledger_impl.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_runtime.rs': (
        'v2_runtime_lifecycle_ordinal_source.rs',
        'v2_runtime_durable_recovery_pending.rs',
        'v2_runtime_effect_ownership_core_impl.rs',
        'v2_runtime_effect_ownership_rebind_impl.rs',
        'v2_runtime_ready_validate_publication.rs',
        'v2_runtime/network_ingress_classification.rs',
        'tests/v2_runtime_pending_binding_cases.rs',
        'tests/v2_runtime_main_00.rs',
        'tests/v2_runtime_main_01.rs',
        'tests/v2_runtime_main_02.rs',
        'tests/v2_runtime_main_03.rs',
        'tests/v2_runtime_main_04.rs',
        'tests/v2_runtime_main_05.rs',
        'tests/v2_runtime_main_06.rs',
        'tests/v2_runtime_unsealed_01b_lifecycle_bounds.rs',
        'tests/v2_runtime_unsealed_02_owner_retirement_and_fairness.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_worker.rs': (
        'v2_worker_completion.rs',
        'v2_worker_io_execution.rs',
        'v2_worker_exact_output.rs',
        'v2_worker_services.rs',
        'v2_worker_services_impl.rs',
        'tests/v2_worker_main_00.rs',
        'tests/v2_worker_main_01.rs',
        'tests/v2_worker_lifecycle_capacity_cases.rs',
        'tests/v2_worker_equivocation_fixture.rs',
        'v2_worker/applied_height_handoff_tests.rs',
        'v2_worker/queue_plan_admission_handoff_tests.rs',
        'v2_worker/upstream_reply_route_test.rs',
        'tests/v2_worker_main_02.rs',
        'tests/v2_worker_main_04.rs',
        'tests/v2_worker_main_05.rs',
        'tests/v2_worker_kagemusha_runtime_gate.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_worker_io_execution.rs': (
        'v2_worker/exact_output_rollover_claim.rs',
        'v2_worker/queue_plan_admission_handoff.rs',
        'v2_worker/exact_output_pending_state.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs': (
        'v2_worker/autonomous_lane_output_reconstruction.rs',
        'v2_worker/kura_replica_advert_refresh.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs': (
        'v2_worker/pending_kura_apply_io_snapshot.rs',
        'v2_worker/current_lane_output_rollover_claim.rs',
        'v2_worker/production_services_drop_impl.rs',
        'v2_worker/effect_services_impl.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_runner.rs': (
        'v2_runner/lifecycle_terminal_recovery.rs',
        'v2_runner/decided_lane_recovery.rs',
        'v2_runner/outer_ingress_cursor.rs',
        'v2_runner/finalized_output_rollover.rs',
        'v2_runner/canonical_recovery_ingress.rs',
        'v2_runner/reply_route_retention.rs',
        'v2_runner/merge_sidecar_recovery.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_runner_tests.rs': (
        'tests/v2_runner_unsealed_00.rs',
        'tests/v2_runner_unsealed_01.rs',
        'tests/v2_runner_unsealed_02.rs',
        'tests/v2_runner_upstream_recovery.rs',
        'tests/v2_runner_lifecycle_startup_order.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_apply.rs': (
        'v2_apply/autonomous_recovery_types.rs',
        'v2_apply/historical_autonomous_recovery.rs',
        'v2_apply/reconciliation_authority.rs',
        'v2_apply/committed_carrier_cleanup.rs',
        'v2_apply/error_recovery.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_core/reducer.rs': (
        'reducer/prepare_certificate_handling.rs',
        'tests/reducer_timeout_and_projection.rs',
        'tests/v2_core_reducer_primitive_projection.rs',
        'reducer/counterfeit_boundary_capability_test.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_core/refinement.rs': (
        'refinement/leader_wire_admission_trace_projection.rs',
        'refinement/first_release_witness.rs',
        'refinement/volatile_summary_well_formed.rs',
        'refinement/post_carrier_transition.rs',
        'refinement_constructor_test_helpers.rs',
        'refinement/transition_gate_tail.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_core/refinement_cases.rs': (
        'refinement_cases/effect_candidate.rs',
        'refinement_cases/terminal_body_pipeline.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_core/tests.rs': (
        'tests/committee_fallback_and_retransmit.rs',
        'tests/v2_core_view_zero_parent_binding.rs',
        'tests/empty_replay_resume_test.rs',
        'tests/delayed_prepare_qc_cache_bounds.rs',
        'tests/v2_core_terminal_transactionality.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_effects.rs': (
        'v2_effects_recovered_fetch_and_pipeline_types.rs',
        'v2_effects_recovered_lifecycle_output_service.rs',
        'v2_effects_lifecycle_admission_settlement.rs',
        'v2_effects_runner_decision_cleanup_plan.rs',
        'v2_effects_test_consumer_wrappers.rs',
        'tests/v2_effects_main_00.rs',
        'tests/v2_effects_main_01.rs',
        'tests/v2_effects_main_02.rs',
        'tests/v2_effects_main_03.rs',
        'tests/v2_effects_main_04.rs',
        'tests/v2_effects_main_05.rs',
        'tests/v2_effects_03_locked_body_and_sidecar.rs',
    ),
    'crates/iroha_core/src/sumeragi/v2_lane_work.rs': (
        'v2_lane_work/canonical_executed_block_application_repair.rs',
        'v2_lane_work/queue_plan_admission_handoff.rs',
        'v2_lane_work/native_amx_signing_guard_capacity_boundary_test.rs',
        'v2_lane_work/typed_finality_handoff_tests.rs',
        'v2_lane_work/terminal_retirement_journal_failure_test.rs',
        'tests/v2_lane_work_native_signing_guard.rs',
        'v2_lane_work/native_amx_route_and_receipt_tests.rs',
        'tests/v2_lane_work_observer_role.rs',
        'tests/v2_lane_work_native_body_recovery.rs',
        'tests/v2_lane_work_lifecycle_and_recovery_cases.rs',
        'v2_lane_work/canonical_executed_block_recovery_drift_test.rs',
        'v2_lane_work/historical_recovery_and_carrier_tests.rs',
        'v2_lane_work_autonomous_ready_durability_tests.rs',
        'v2_lane_work/autonomous_retirement_and_merge_tests.rs',
        'v2_lane_work/queue_plan_admission_handoff_tests.rs',
    ),
    'crates/iroha_core/src/sumeragi/tests/v2_lane_work_lifecycle_and_recovery_cases.rs': (
        'v2_lane_work_effect_queue.rs',
    ),
    'integration_tests/tests/sumeragi_v2_runner.rs': (
        'sumeragi_v2_runner/prepare_qc_split_tests.rs',
        'sumeragi_v2_runner/status_validation_helpers.rs',
        'sumeragi_v2_runner/status_set_validation.rs',
    ),
    'integration_tests/tests/sumeragi_v2_runner/prepare_qc_split_tests.rs': (
        'restart_timing_test.rs',
    ),
    'crates/iroha_sumeragi_core/src/verus_proofs.rs': (
        'verus_proofs/production_transition_contracts.rs',
        'verus_proofs/in_flight_first_release_proofs.rs',
        'verus_proofs/production_kernel_tail.rs',
    ),
    'crates/iroha_core/src/sumeragi/tests/v2_adapter_main_00.rs': (
        'v2_adapter_activation_context.rs',
    ),
    'crates/iroha_core/src/sumeragi/tests/v2_adapter_main_03.rs': (
        'v2_adapter_04_wal_recovery.rs',
        'v2_adapter_04b_lifecycle_startup.rs',
        'v2_adapter_05_direct_lifecycle.rs',
    ),
    'crates/iroha_core/src/sumeragi/tests/v2_adapter_05_direct_lifecycle.rs': (
        'v2_adapter_05_direct_lifecycle_recovered_wal_seal_case.rs',
    ),
    'crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery.rs': (
        'v2_adapter_04_wal_recovery_decision_classifier_cases.rs',
    ),
    'crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs': (
        'v2_adapter_04b_lifecycle_startup_tail.rs',
    ),
    'crates/iroha_core/src/sumeragi/tests/v2_adapter_main_04.rs': (
        'v2_adapter_01_replay_and_registry.rs',
        'v2_adapter_02_view_and_lock_progress.rs',
        'v2_adapter_03_tc_and_terminal_ingress.rs',
    ),
    'crates/iroha_core/src/sumeragi/tests/v2_worker_main_01.rs': (
        'v2_worker_reply_route_cases.rs',
        'v2_worker_backpressure_cases.rs',
        'v2_worker_recovered_lifecycle_output_cases.rs',
        'v2_worker_nonzero_view_restart.rs',
    ),
    'crates/iroha_core/src/sumeragi/tests/v2_worker_backpressure_cases.rs': (
        'v2_worker_backpressure_retirement_cases.rs',
    ),
    'crates/iroha_core/src/sumeragi/tests/v2_effects_main_05.rs': (
        'v2_effects_kura_tip_replay.rs',
        'v2_effects_01_view_churn_and_runtime_steps.rs',
        'v2_effects_highest_prepare_retention.rs',
        'v2_effects_02_admission_handoffs.rs',
    ),
}
def _read_reviewed_rust_source_fixture(
    repo_root: Path,
    relative: str,
    errors: list[str],
    description: str,
    expanded_components: tuple[str, ...] | None = None,
    _expansion_stack: tuple[str, ...] = (),
) -> tuple[Path, str]:
    """Expand a non-Git mutation fixture using the reviewed recursive manifest."""

    path = repo_root / relative
    if relative in _expansion_stack:
        cycle = _expansion_stack + (relative,)
        errors.append(f"{path}: reviewed Rust include fixture cycle through {cycle!r}")
        return path, ""
    if not path.is_file() or path.is_symlink():
        errors.append(f"{path}: {description} must be a regular non-symlink file")
        return path, ""
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        errors.append(f"{path}: cannot read {description}: {error}")
        return path, ""
    manifest = _REVIEWED_RUST_INCLUDE_MANIFESTS.get(relative)
    if manifest is None:
        return path, source
    if expanded_components is not None:
        unknown_components = tuple(
            component
            for component in expanded_components
            if component not in manifest
        )
        if unknown_components:
            errors.append(
                f"{path}: requested unknown reviewed Rust include components "
                f"{unknown_components!r}"
            )
    initial_error_count = len(errors)
    provider_invocations = (
        _RECURSIVE_REVIEWED_RUST_SOURCE._rust_provider_invocations(
            source, path, manifest, errors,
        )
    )
    if len(errors) != initial_error_count:
        return path, ""
    observed = tuple(invocation.relative for invocation in provider_invocations)
    if observed != manifest:
        errors.append(
            f"{path}: reviewed Rust include inventory must equal {manifest!r}; "
            f"found {observed!r} across {len(provider_invocations)} code-level "
            "binding(s)"
        )
    component_sources: dict[str, str] = {}
    for component_relative in manifest:
        component_path = path.parent / component_relative
        if not component_path.is_file() or component_path.is_symlink():
            errors.append(
                f"{component_path}: reviewed Rust include component for {path} "
                "must be a regular non-symlink file"
            )
            component_source = ""
        else:
            try:
                component_source = component_path.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError) as error:
                errors.append(
                    f"{component_path}: cannot read reviewed Rust include "
                    f"component for {path}: {error}"
                )
                component_source = ""
        if expanded_components is None or component_relative in expanded_components:
            component_repo_relative = Path(relative).parent / component_relative
            component_repo_relative = component_repo_relative.as_posix()
            if component_repo_relative in _REVIEWED_RUST_INCLUDE_MANIFESTS:
                _, component_source = _read_reviewed_rust_source_fixture(
                    repo_root, component_repo_relative, errors, description,
                    _expansion_stack=_expansion_stack + (relative,),
                )
        component_sources[component_relative] = component_source
    expanded: list[str] = []
    cursor = 0
    for invocation in provider_invocations:
        expanded.append(source[cursor:invocation.end])
        component_relative = invocation.relative
        component_source = component_sources.get(component_relative)
        if component_source is None or (
            expanded_components is not None
            and component_relative not in expanded_components
        ):
            cursor = invocation.end
            continue
        if expanded[-1] and not expanded[-1].endswith("\n"):
            expanded.append("\n")
        expanded.append(component_source)
        if component_source and not component_source.endswith("\n"):
            expanded.append("\n")
        cursor = invocation.end
    expanded.append(source[cursor:])
    return path, "".join(expanded)


def _read_reviewed_rust_source(
    repo_root: Path,
    relative: str,
    errors: list[str],
    description: str,
    expanded_components: tuple[str, ...] | None = None,
) -> tuple[Path, str]:
    """Read one Rust source through the authenticated recursive resolver.

    Production validation always uses the shared stage-aware recursive closure.
    Synthetic mutation fixtures are intentionally not Git worktrees, so their
    already-reviewed direct components retain the narrow fixture reader.
    """
    if expanded_components is not None or repo_root.resolve() != ROOT_DIR.resolve():
        return _read_reviewed_rust_source_fixture(
            repo_root,
            relative,
            errors,
            description,
            expanded_components,
        )
    closure = _RECURSIVE_REVIEWED_RUST_SOURCE._resolve_reviewed_rust_source(
        repo_root,
        relative,
        description,
        errors,
    )
    path = repo_root / relative
    if closure is None:
        return path, ""
    return closure.path, closure.source


def _read_cross_tool_source_item_seal_source(
    repo_root: Path,
    relative: str,
    cache: dict[str, tuple[Path, str]],
) -> tuple[Path, str]:
    """Authenticate and memoize one cross-tool source-item seal provider."""

    reviewed = cache.get(relative)
    if reviewed is not None:
        return reviewed
    errors: list[str] = []
    reviewed = _read_reviewed_rust_source(
        repo_root, relative, errors, f"cross-tool source seal {relative}"
    )
    if errors:
        raise ValueError(
            f"cross-tool source seal could not authenticate {reviewed[0]}: "
            + "; ".join(errors)
        )
    cache[relative] = reviewed
    return reviewed


@_RECURSIVE_REVIEWED_RUST_SOURCE._reviewed_rust_source_cache()
def _reviewed_rust_include_manifest_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Fail closed unless every reviewed split Rust source has its exact closure."""

    errors: list[str] = []
    reviewed_paths = tuple(
        dict.fromkeys(
            path
            for parent_relative, component_relatives in _REVIEWED_RUST_INCLUDE_MANIFESTS.items()
            for path in (
                parent_relative,
                *(
                    (Path(parent_relative).parent / component_relative).as_posix()
                    for component_relative in component_relatives
                ),
            )
        )
    )
    try:
        tracked_result = subprocess.run(
            ["git", "-C", str(repo_root), "ls-files", "--stage", "-z", "--", *reviewed_paths],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        errors.append(
            f"{repo_root}: cannot authenticate reviewed Rust include Git tracking: {error}"
        )
        tracked_paths: set[str] = set()
    else:
        if tracked_result.returncode != 0:
            detail = tracked_result.stderr.decode("utf-8", errors="replace").strip()
            errors.append(
                f"{repo_root}: cannot authenticate reviewed Rust include Git tracking: "
                f"{detail or f'git ls-files exited {tracked_result.returncode}'}"
            )
            tracked_paths = set()
        else:
            tracked_paths = {
                record.split(b"\t", 1)[1].decode("utf-8", errors="surrogateescape")
                for record in tracked_result.stdout.split(b"\0")
                if b"\t" in record
            }
    for relative in reviewed_paths:
        if relative not in tracked_paths:
            errors.append(
                f"{repo_root / relative}: reviewed Rust source must be Git-tracked"
            )
    for relative in _REVIEWED_RUST_INCLUDE_MANIFESTS:
        _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            "reviewed split Rust source",
        )
    return errors

def _kura_production_source_inventory(
    repo_root: Path = ROOT_DIR,
) -> tuple[
    Path,
    str,
    tuple[tuple[str, Path, str], ...],
    list[str],
]:
    """Load the exact direct production include closure of ``kura.rs``."""

    source_root = repo_root / "crates" / "iroha_core" / "src"
    kura_path = source_root / "kura.rs"
    errors: list[str] = []
    if not kura_path.is_file() or kura_path.is_symlink():
        errors.append(
            f"{kura_path}: Kura production source inventory root must be a "
            "regular file"
        )
        kura_source = ""
    else:
        try:
            kura_source = kura_path.read_text(encoding="utf-8")
        except OSError as error:
            errors.append(
                f"{kura_path}: cannot read Kura production source inventory "
                f"root: {error}"
            )
            kura_source = ""

    structural_source = mask_rust_comments_and_literals(kura_source)
    exact_test_module_markers = tuple(
        re.finditer(
            r"(?m)^#\[cfg\(test\)\]\n"
            r"pub\(crate\) mod tests (?P<open>\{)[ \t]*$",
            structural_source,
        )
    )
    test_module_candidates = tuple(
        re.finditer(
            r"(?m)^#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*"
            r"(?:(?:pub(?:\s*\([^\r\n)]*\))?)\s+)?"
            r"mod\s+tests\s*\{",
            structural_source,
        )
    )
    if len(exact_test_module_markers) != 1 or len(test_module_candidates) != 1:
        errors.append(
            f"{kura_path}: Kura production source inventory must retain "
            "exactly one terminal cfg(test) module boundary in the exact "
            "`#[cfg(test)]\\npub(crate) mod tests {` form; found "
            f"{len(exact_test_module_markers)} exact boundary marker(s) "
            f"across {len(test_module_candidates)} cfg(test) tests-module "
            "candidate(s)"
        )
        production_source = kura_source
    else:
        test_module_marker = exact_test_module_markers[0]
        production_source = kura_source[: test_module_marker.start()]
        depth = 0
        test_module_close = None
        for offset in range(
            test_module_marker.start("open"), len(structural_source)
        ):
            token = structural_source[offset]
            if token == "{":
                depth += 1
            elif token == "}":
                depth -= 1
                if depth == 0:
                    test_module_close = offset
                    break
        if (
            test_module_close is None
            or structural_source[test_module_close + 1 :].strip()
        ):
            errors.append(
                f"{kura_path}: Kura cfg(test) tests module boundary must own "
                "the terminal source suffix"
            )

    include_parse_errors: list[str] = []
    include_invocations = _RECURSIVE_REVIEWED_RUST_SOURCE._rust_include_invocations(
        production_source,
        kura_path,
        include_parse_errors,
    )
    errors.extend(include_parse_errors)
    observed_components = tuple(
        invocation.relative for invocation in include_invocations
    )
    production_test_components = tuple(
        component
        for component in observed_components
        if component.startswith("kura/tests/")
    )
    if production_test_components:
        errors.append(
            f"{kura_path}: Kura production source must end before all test "
            f"includes; found {production_test_components!r} before the exact "
            "terminal cfg(test) module boundary"
        )
    if (
        observed_components != _KURA_PRODUCTION_COMPONENT_FILES
        or len(include_invocations) != len(_KURA_PRODUCTION_COMPONENT_FILES)
    ):
        errors.append(
            f"{kura_path}: Kura direct production include inventory must equal "
            f"{_KURA_PRODUCTION_COMPONENT_FILES!r}; found "
            f"{observed_components!r} across {len(include_invocations)} "
            "include invocation(s)"
        )

    components: list[tuple[str, Path, str]] = []
    for relative in _KURA_PRODUCTION_COMPONENT_FILES:
        path = source_root / relative
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: Kura production source inventory component must be "
                "a regular non-symlink file"
            )
            source = ""
        else:
            try:
                source = path.read_text(encoding="utf-8")
            except OSError as error:
                errors.append(
                    f"{path}: cannot read Kura production source inventory "
                    f"component: {error}"
                )
                source = ""
        components.append((relative, path, source))
    return kura_path, kura_source, tuple(components), errors

# Exact token-stream digests for the nontrivial Verus projection theorems and
# their concrete mutation witnesses. Unlike raw substrings, these bind the
# declaration, contract, and proof body of one real, context-checked item while
# remaining insensitive to comments, literals, and formatting.
_PRODUCTION_CAUSAL_FIFO_VERUS_ITEM_SHA256 = {
    "production_stable_subsequence": (
        "820ed299168153274ca3457e3446bd8bd2c433b49286bd9f5e4c57975af252ba"
    ),
    "production_fresh_causal_successors_excludes_prior_owners": (
        "b6236603215e29cabfc1a44d7495ad101d0390f23863127714354bb2d89b0c56"
    ),
    "production_fresh_causal_successors_keeps_every_fresh_value": (
        "0ff2376b08c3a96cb0c82f3d87404796a4d863f919c9cf58ddee5858c3ebf898"
    ),
    "production_fresh_causal_successors_has_unique_values": (
        "030db5f75b6cbddc32ba57aa8eac66169b993b3b067105232005d0985ee83daa"
    ),
    "production_fresh_causal_successors_preserves_first_owner_order": (
        "14ef7781949552882dda3fb35f066ea89e0a943bb40dcca2b7464b0a05d81109"
    ),
    "production_async_causal_fifo_after_batch_preserves_fresh_tail": (
        "ef11350ee44c0551a2a34f091f3197559a684075b3c86a28d0d4a9528acc6a6e"
    ),
    "production_inverted_owner_filter_mutant": (
        "550fa68ae21e42cf9bd1c9756e229ed40b23e28ebc417cd7907d9b805e634bec"
    ),
    "production_inverted_owner_filter_mutant_is_rejected": (
        "898288a0767a678bd7080abe09995de2ec2e7c406c796ab79202b28e200d0a41"
    ),
    "production_reversed_fresh_order_mutant": (
        "e14bf30fb8489b4922e8f81c79e305a0f2c07d57f2abd3101bff581c915760a7"
    ),
    "production_reversed_fresh_order_mutant_is_rejected": (
        "a0b995d9442eb0fef330eb7383c258f16dab35b61b84e02716437f758214a6f5"
    ),
    "production_completion_capacity_product_rank": (
        _CHECKED_PRODUCTION_COMPLETION_PRODUCT_RANK_SHA256
    ),
    "production_completion_capacity_product_rank_descends": (
        _CHECKED_PRODUCTION_COMPLETION_PRODUCT_RANK_DESCENT_SHA256
    ),
}

_PRODUCTION_EFFECT_CANDIDATE_TLA_OPERATOR_SHA256 = {
    "AsyncCausalCandidateLifecycleCapacity": (
        "02882bb4b0f0b52c5e9d0994bdb8f459b768e7612851b68956932bbaecf93591"
    ),
    "AsyncCandidateCausalOrigin": (
        "00a780f6b919c66fc03507311341ff7590f4d37cba9bc4711ee73ec29e23eeef"
    ),
    "ExactAsyncCandidateIdentity": (
        "ceeaebbac450ef309c949f0fc399393ea441763fbb91948bdd5bb8c89f9a165f"
    ),
    "FreshCommandSuccessors": (
        "387d64f773b0d6df95e41ce65e108bd37e23c6b3574ecdd0f049b242b2235997"
    ),
    "AppendCausalSuccessors": (
        "f4c500c716af8d23357691a07a603700fa1d762b2192a2f310845ea021bc5781"
    ),
}

_PRODUCTION_EFFECT_CANDIDATE_TLA_THEOREM_SHA256 = {
    "CommandSuccessorsRetainCausalOrigin": (
        "cf52b0688a75691dc706a787f2f776f71d929013d82566ddaead6cfb36602247"
    ),
}

_PRODUCTION_COMPLETION_CAPACITY_TLA_OPERATOR_SHA256 = {
    "Stage6CompletionCapacityGoal": (
        "c983618885f38d74a140a2e1d03f0e9e2e1721553db0e0e457efbf67dbc41dde"
    ),
}

_PRODUCTION_COMPLETION_CAPACITY_TLA_THEOREM_SHA256 = {
    "FairStage6CompletionCapacityOpens": (
        "d6395e9f9616aca03edbd2dffde7e3aa340e80e3be9fe333f8c4a362ef2e21c1"
    ),
}

# Exact comment/literal-free token digests for the production Rust items whose
# control flow establishes the bounded persisted-continuation, one-transition
# deferred scheduling, and TC recovery seam.
# These whole-item bindings prevent an attacker from preserving the reviewed
# snippets in a dead branch while changing the executable path around them.
_PRODUCTION_CAUSAL_FIFO_RUST_ITEM_SHA256 = {
    "from_record": (
        "399605c9add8c3bf579fc03c6b46b52533c1ca726cc175fcd80cd33536882ce7"
    ),
    "budget": (
        "db8200b278278f301d4a254f04ca49d42353b486baac55c104cb4461ba5d125f"
    ),
    "deferred_work_is_serviceable": (
        "1036f93b127acad65512fcb095d7f19bdc547f93d22ebebbd227277a0a30405c"
    ),
    "completion_unblocks_deferred_fence": (
        "0964e3e5d76ef3f72a240c03c46c3c15a71c52572acaa34784276c97858d3ab7"
    ),
    "command_is_blocked_by_deferred_fence": (
        "9bdfac81a0ee1fa04e14027f586231bde5f1127edc8b37b7ab1d719c90aa43f8"
    ),
    "authenticated_command_reaches_fenced_reducer": (
        "0db0ad51e049c43f83a194f9f1c05440ac59e87d883f35b24e54f28affba61cf"
    ),
    "ready_to_finish": (
        "039df6143d8b4505489db3c75ab2d3d24c01a30a4da0f0d277d865f44cfa0ffa"
    ),
    "drain_deferred_with_evidence": (
        _SERVICED_CANDIDATE_PRODUCTION_ITEM_SHA256[
            "drain_deferred_with_evidence"
        ]
    ),
    "fail_deferred_service_contract": (
        "b3ca797528869db777b292d74ccba9425cced620e3d56f429ffdeb624b285799"
    ),
    "step_reducer": (
        "c9c5192fe1e4b7042a76aced8da8fcbafe51d4ed4a0f5ad6608c0b81492864a6"
    ),
    "drive_effects": (
        "228291c466d581465006e2b731591caea95750c08dd0c5cea836b8d3c7de6e95"
    ),
    "pop_fence_dependency_with_ownership": (
        "cf5a9312b16db72176857a1dfad4e7b64636f7bebd7edc7142f052ec3a2e567b"
    ),
    "fence_blocked_occurrence_owners": (
        "45b2891248d62b2a998ebcaf2e29d35bfeb1687a8869a0b27776904ee6b3f778"
    ),
    "freeze_due_clock_owners": (
        "d1c028eb58483adffe8eb1415b431d3f031714167535af7382d3ee39b5cf4027"
    ),
    "validate_clock_owner_physical_cuts": (
        "0c32bff217ddf2b631eba89320a3b6fad6dda20830ed9e0ed7778ab47d2185d6"
    ),
    "clock_owner_reservation_blockers_occurrence": (
        "940ef5e22bf2e2a1d9ef78fca48d2517d6294259b1b8108892420d0de78ba34e"
    ),
    "clock_owner_reservation_blocks_occurrence": (
        "9fa4051663d34eab5a8ecbfc1192ca92789ca710085e95726a796a29b20450da"
    ),
    "clock_owner_reservation_blocks": (
        "f6db1825402a97f52f35c0ef7cabda380be9055ca7507dfae0476d339057304c"
    ),
    "enqueue_after_clock_reservation": (
        "d71a57ec0eb41724bb65040a35e9d0ac308a2ef387868c207070add6e02be541"
    ),
    "scheduler_arbitration_inputs": (
        "79fc6e55d53b331ff01aab01f638cfb3238f607115ca56a4a8cbd423f3572028"
    ),
    "try_step_pre_timeout_locked_prepare_qc": (
        "b2555b96fe2f6ff8a1c1f2cfc6e8738df498942e1bbf5cebbd6a1275619b29bb"
    ),
    "minimum_active_lifecycle_ordinal": (
        "bb4ac2c885dce0086aed3df676af4b5d4c45ea00c9d93e06521242058ef85c9d"
    ),
    "minimum_active_lifecycle_ordinal_excluding": (
        "bb53e945b76e2bdbd83e60c3e259d6a946eaf83be9685ca099c01fc8da5dbdf8"
    ),
    "minimum_active_lifecycle_ordinal_for_deferred": (
        "1d27b616e3804e98006a4353d9c7d1e03005ce9299c2e0b0ccbb7782209210df"
    ),
    "minimum_active_lifecycle_ordinal_for_deferred_excluding": (
        "eca0b361eb0d72cf5e92e3d38563e04efe062ccde0ffd0db0d75942a40e12505"
    ),
    "eligible_deferred_admission_ordinals": (
        "ca2db5b8e601e556e10001d3c0a6be5b18dd1cd1f92e5eecf0273fe158fb232d"
    ),
    "runtime_step": "5963ef7728c8c6555bf7156b64cc649ee7c54b5a77befae64db17e24cdc4d313",
    "dispatch_one_adapter_deferred": (
        "a4c901cdd676731f6cfd3c4dcb52718df366f65bc7ca5e8d1a54a841ec30cdab"
    ),
    "dispatch_one_fence_dependency": (
        "539239fa96fca8ea08dc56ac89041b7be0bb6f5f3d33f65259ad8e7833173b69"
    ),
    "real_adapter_fence_completion_bypasses_only_preowned_fenced_fifo": (
        "682c243806ba2c813238722e16e9506ad1534fac572a18c29a3d270a9157ec51"
    ),
    "fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers": (
        "9beb353187babef7076f623cd56f55030341e464604c70ce6b8c7a8fff89cc81"
    ),
    "tc_promoted_lock_requires_same_subject_reproposal_before_commit": (
        "54137f436b2e9b154a0875ca4b0ebbbc998a1e0f6dea39d8480fd8b6eb0106b0"
    ),
}

# Structural inventory for the process-local capability chain which binds an
# exceptional fence/pacemaker selection to one immutable physical queue
# occurrence and one exact signer incarnation. Exact token digests are kept in
# the reviewed SHA-256 tables once refreshed; this inventory independently
# prevents a helper from disappearing from the proof-ledger fidelity pass.
_PRODUCTION_CAUSAL_FIFO_NONFORGEABLE_HELPER_CHAIN = (
    "cached_queue_occurrence_owner",
    "ownership_snapshot",
    "mint_selection_seal",
    "contains_queue_occurrence_owner",
    "restore_selected_command",
    "oldest_active_lifecycle_ordinal_before_physical_cut_excluding",
    "physically_eligible_deferred_admission_ordinals",
    "pop_pacemaker_progress_with_ownership",
    "runtime_view_blocked_progress_authorization_projection_hash",
    "wire_payload_matches_current_strict_timeout_recovery_round",
    "wire_payload_advances_or_supersedes_future_prepare_qc_fifo_block",
    "validates_retained_blocker",
    "runtime_queue_occurrence_owner_projection_hash",
    "runtime_queue_ownership_snapshot_projection_hash",
    "runtime_queue_occurrence_set_matches_snapshot",
    "runtime_queue_selection_seal_projection_hash",
    "matches_scheduler_occurrence",
    "runtime_scheduler_projection_hash",
    "current_signature_fence_identity",
    "clear_fence_retry_blocked_fifo_owners",
    "reconcile_fence_retry_blocked_fifo_owners",
    "retain_fence_retry_blocked_fifo_owner",
    "retain_scheduler_ownership",
    "try_step_pacemaker_escape",
    "dispatch_one_pacemaker_progress",
)

_PRODUCTION_CAUSAL_FIFO_NONFORGEABLE_ITEM_SHA256 = {
    "cached_queue_occurrence_owner": (
        "a113678ffffef7c7bfae9ba827bfddff80e069dd1c1e84d9dbb5525e99138103"
    ),
    "ownership_snapshot": (
        "b46fe089ce520f215926646c49fb991755ae3e2ba6b4dda5242808df6f8d0645"
    ),
    "mint_selection_seal": (
        "3f4a3eaaa3f1a703392d04b9c05262f2638c2bb3d4f65a9e247b83a0c1a44af8"
    ),
    "contains_queue_occurrence_owner": (
        "c116a2a5423be69a43d6dbf13c00525a416f974020cd96ad7f55282a27293a98"
    ),
    "restore_selected_command": (
        "af1497f566c0c9547983f23469490478ecb322286899a44d7f2761dd668af3a4"
    ),
    "oldest_active_lifecycle_ordinal_before_physical_cut_excluding": (
        "469e352d63c31ef4776fa31bfce7484c16a33f00982058005317494fd87f9e17"
    ),
    "physically_eligible_deferred_admission_ordinals": (
        "0b51819b618a5d50f632ffb772a94be1560e960b57746c10ba3e077aa97b60d3"
    ),
    "pop_pacemaker_progress_with_ownership": (
        "326f99b8605bbaceb6cc2328bc6235b77936cba4ad20b4011ccca08e5f6e1793"
    ),
    "runtime_view_blocked_progress_authorization_projection_hash": (
        "e9a29e0a277bbae16f4b1ef8219bdf892f79ce733b56bbe01bd2a9f7c5c36158"
    ),
    "wire_payload_matches_current_strict_timeout_recovery_round": (
        "68e1d7b8e1c16bca7c17b81a0cff0ef1709aa66d2a952d50c5b8d928835cd458"
    ),
    "wire_payload_advances_or_supersedes_future_prepare_qc_fifo_block": (
        "46f58c62ecc82cea8aa67cc906a176cc38782b6e135bd5dbb62a60327cff5515"
    ),
    "validates_retained_blocker": (
        "e1b3b5823bf3c81749298d436f04c1bdf1d8d0b06b3f1d26693089aab119a05b"
    ),
    "runtime_queue_occurrence_owner_projection_hash": (
        "c220a852adcdb9e7a2a20cec297ee79a87cf5b88acac55410eac3c824614d1a6"
    ),
    "runtime_queue_ownership_snapshot_projection_hash": (
        "bb6d0062be867b73c5269979c4f850bc9614e8f15c76f65c60dbc9e44d497eb9"
    ),
    "runtime_queue_occurrence_set_matches_snapshot": (
        "c89fd5e9e3df83fb51e201555e53799c625c9978932fb61be844476ad2c977be"
    ),
    "runtime_queue_selection_seal_projection_hash": (
        "b9036732219388125a51d4652ce4143aaefe00574d7f9f9f359cc90b76b56368"
    ),
    "matches_scheduler_occurrence": (
        "355052d58f6acbf00d4c7164d0e54a3e13044a087150cd4d56643110f2d51ed6"
    ),
    "runtime_scheduler_projection_hash": (
        "415a47652facf9236a0205764edc92f3eb545f35e2f83b9a3562a417bf77c1ba"
    ),
    "current_signature_fence_identity": (
        "d3e7b714af5442d66ec2e8f4e1c57bfe649c5b8f41b310d71929fa6586baf0ed"
    ),
    "clear_fence_retry_blocked_fifo_owners": (
        "2ce5c0a06b37a5342eaa297e3fef4ae70ea0856344f03c8eaaf08fe5488097ae"
    ),
    "reconcile_fence_retry_blocked_fifo_owners": (
        "d603d692ea6053b116efce0eb77fb0872e2f0277480239a5a4183583015de370"
    ),
    "retain_fence_retry_blocked_fifo_owner": (
        "f6c5e5ac82dfda26d972715a073a2ddb7a41135ef9be0ee02ed77944f9e186f8"
    ),
    "retain_scheduler_ownership": (
        "d333ce49089e9a1d7a15a9111d0ee6541fce2171f4af835249095e6a71aa0929"
    ),
    "try_step_pacemaker_escape": (
        "55cd8225900a605ce0821bda634c5247d007b53db561c404b4be2de13a0ac9fe"
    ),
    "dispatch_one_pacemaker_progress": (
        "d806941928afcb15ee72ed7eef771ac10cad55a7b714827b49052617b76c894d"
    ),
    "occurrence_owner_validate_exact": (
        "0f018cbe8ab36b2a5e1f3aa128d7c22c367083b399184aedc8c829f9228db8e3"
    ),
    "occurrence_owner_matches_queued": (
        "e8544136325028c614b22287649c3e7af5779669328b0b64918093edabb8302a"
    ),
    "queue_snapshot_validate_identity": (
        "9ee5bf23ffa531c168a3722a216ef32a9c2cf83f4265e25dde0fe750b4fa890f"
    ),
    "queue_selection_validate_identity": (
        "94d5ca84828c28e62e55af3682b290e5d02eb1db477243ae0a25ad55c39d1542"
    ),
    "queue_selection_kind_code": (
        "08b07eaf05cd85228dc54c2f49a2b54071814c916a19e92cb3cff56fd5eb2518"
    ),
    "view_blocked_authorization_struct": (
        "fec50a1d9938ec78bdf262bb973e4943802843a0d4a48c0be4bf2624dff3c0dd"
    ),
    "scheduler_arbitration_inputs_struct": (
        "af1222460a1e16f0e7d60b90ef34fc28a768185538f554bc0281cba85bb2a9a8"
    ),
    "scheduler_ownership_evidence_struct": (
        "ea80f8f1f216fe71d8219279593dd64b86ba13afec0cfb86ac1b46b6a4a1a903"
    ),
    "queue_selection_matches_scheduler_occurrence": (
        "355052d58f6acbf00d4c7164d0e54a3e13044a087150cd4d56643110f2d51ed6"
    ),
    "scheduler_evidence_validate_exact": (
        "458ee09d659891d8f30a77a2b2c49a564914ade84ee013e9be77a8aa89f59459"
    ),
    "view_blocked_authorization_new": (
        "7c4cfb17b4841105a0bc2c7e966acbd72ea4c8c232cd502918f4136c9f8717ad"
    ),
    "bounded_ingress_ordinary_view_blocked_progress_authorization": (
        "432f1bd53d20501e9fa004f394aa871f49eb7144c10b82aab0bbeb311c7688ee"
    ),
    "runtime_ordinary_view_blocked_progress_authorization": (
        "1f145ba32cfdf6e21d078255e660ccfecc7de2fff4fd4a718fceeec24669511e"
    ),
    "runtime_driver_default_pacemaker_progress_blocked_target_view": (
        "881e840c28227633ad82e695d9c1214352c16d6622cb9a0c57237b299ccb5424"
    ),
    "runtime_driver_default_pacemaker_progress_releases_view_block": (
        "ecd3f05b7cff39a5654e509bee842092e9e39f7e96d01e188e067a7e44b4cab9"
    ),
    "runtime_driver_pacemaker_progress_blocked_target_view": (
        "d667e1e855dd787defdaa559d4a3cfaef26f5b15cedf8e505184f7733db8495a"
    ),
    "runtime_driver_pacemaker_progress_releases_view_block": (
        "dabc25f07825611ec62cef6aeb5e46aa1a6ad1e52dac26d3672ae714f0a005bd"
    ),
    "adapter_pacemaker_escape_is_parked": (
        "5513bb3477396c268d3dc7ed75ded1d6c027bc5780c5f129b53920720c498d86"
    ),
    "adapter_signature_fence_is_active": (
        "2012f7befb770adf8ab34a94452c592836dc3ae6c8e4e8ca4c3148f745e4db38"
    ),
    "adapter_signature_fence_identity": (
        "3bfa48370472e85c7727695165fae3c6758a4a9ff228111c806e558929f7faad"
    ),
    "runtime_driver_certified_progress_bypasses_signature_fence": (
        "6c6761d3f63a292cf04fd123fc055891200081b7dda8f211482f58cfef7b385d"
    ),
    "runtime_driver_completion_unblocks_deferred_fence": (
        "1379d6ae2714aa4bfc2b02cec74611a0f65799319ab94b8fc37b04ef0fec62a5"
    ),
    "runtime_driver_command_is_blocked_by_deferred_fence": (
        "63b773eb42e48fc13be96b5aecd992d9e23a264ba28d0720e450eace4578dc5d"
    ),
    "runtime_driver_default_command_matches_deferred_authenticated_owner": (
        "f7890a72aa8fe05e4f16321614d6fc96b31de12353b923fe34724a1e46109b61"
    ),
    "runtime_driver_pacemaker_escape_is_parked": (
        "4ed5db3024338e14145b7186c4392feb1c14348e887f3695d813f980ad6680e7"
    ),
    "runtime_driver_signature_fence_is_active": (
        "34aa7d6b66cf8ad39f348f2b0ebecd773f4c771867951769222215f5ab877294"
    ),
    "runtime_driver_signature_fence_identity": (
        "135fcb20dbb91a2adfa807031ee70a6b11e396b8369959dbeca22a8aba2718f7"
    ),
}

# Exact comment/literal-free token digests for the production ownership bridge
# which retains the complete canonical envelope and fair-ingress carrier from
# authenticated admission through Busy-deferred service.
_RUNTIME_ENQUEUE_NETWORK_WITH_INGRESS_OWNERSHIP_ITEM_SHA256 = (
    "8b18f6a31269395d3545bf9e96149b7c28df055e2a4da26880c43db429ba1287"
)
_RUNTIME_RESTORED_PRE_RUNTIME_TC_CANNOT_DEADLOCK_ITEM_SHA256 = (
    "6f698df89965d5ec5c98058d48015c07c65c50335fa2db4d237b49882015ce8d"
)
_RUNNER_ROLLOVER_FINALIZED_HEIGHT_OUTPUTS_ITEM_SHA256 = (
    "57da29721e6df2151d024228810103c3e65c04789a7ac19b06d8a39b705b6104"
)

_AUTHENTICATED_DEFERRED_OWNERSHIP_RUST_ITEM_SHA256 = {
    "matches_authenticated_runtime_bytes": (
        "1d5bd7b516504865f7f8f8db0416c7c1d337ba83342cbd66b564b24e46df3870"
    ),
    "deferred_authenticated_message_owner": (
        "b8279dae9cd51a72cc4a84cd80063fc5a90b2ca72066b48f59571166b560cc8b"
    ),
    "authenticated_deferred_admission_ordinals": (
        "56e2d8e09f770616ace0c2421e6105c7ee2cf8d7428b4016003b5aa5b71bf271"
    ),
    "deferred_authenticated_event_matches_wire": (
        "71cff12249ba75d45cc55f3be85c966fa2f317a3638ce36fa250d399c0f88fd5"
    ),
    "wire_ingress_missing_execution_commitment": (
        "ab4345a4067a48f67735cb5867d717cf8f32aa809f7218aeb04c8eaaf3775678"
    ),
    "runtime_ingress_from_fair_ingress": (
        "638d44eae201d3477a987e857d3c9318c7a525347cddb9d5bbce704d9bbc7985"
    ),
    "runtime_ingress_validate_exact": (
        "87a8e78b3de06372da6678ca45737fa8874d5b9c3aac537d0db55b7848715a41"
    ),
    "runtime_ingress_leader_wire_physical_carrier": (
        "964b47b38e48c18e93b0a4e9d63ac711f5ef14ed89d933ffbe1e93233d59d7c5"
    ),
    "runtime_ingress_is_physical_leader_wire_replay": (
        "7af77803b6cb064c9f611601e930c2ee13db7e961a5c85fad10a80900299792a"
    ),
    "runtime_ingress_earliest_physical_carrier": (
        "aab95bf9cc5928cda4550fa375c56f8958b473940ab865b7527946eb5f235325"
    ),
    "runtime_ingress_contains_physical_carrier": (
        "6554701b2dcc0d85a2006eabf89f0c538526fd99cd9a78456c031d79c5aec1bf"
    ),
    "runtime_ingress_validate_frozen_physical": (
        "51990098b63ed265a79a1617ff4e22c54a1a9eb740efea948df9950ba8f4a1b5"
    ),
    "runtime_ingress_exactly_matches_authenticated": (
        "bd5cfafeb6e9ea0c7a2bd11e92a3cf9e49e8b952bd33586389ff01d751cf8d57"
    ),
    "runtime_ingress_can_merge_downstream": (
        "0a9b6424c76b8d53a6e630431e3e9153d20e25d755abbe38384c5f59e9064553"
    ),
    "runtime_ingress_merge_downstream": (
        "55f59286a356c0cd1c47b572db72660dcdedcc40f3946c2e3b62c7eb5c805e92"
    ),
    "runtime_driver_dispatch": (
        "1344a8e164c53bf7b79b3b7e4622c7c19ebdc45581683b5c7acc20ec0109d114"
    ),
    "runtime_driver_dispatch_deferred": (
        "f80c271a6766106b620e6b0dcbd7fb3a37db63fece634c09818ddb74498202d9"
    ),
    "runtime_driver_deferred_occurrence_ownership": (
        "be42df5e4cfeb06f2887a6d713b06c4a0edb4acc15e6f1bafe8e1454bf58ea91"
    ),
    "runtime_driver_seal_deferred_runtime_ownership": (
        "2948fe28ee3912010c5f0628e6688a329de631c8c5c3f9456178c17e7f58358a"
    ),
    "runtime_driver_command_matches_deferred_authenticated_owner": (
        "f9c9c0bbb2ac3f95d158211d85acd7f10320bcbeadb9f597b18e81c615d80ac6"
    ),
    "runtime_ingress_physical_ownership_validate_exact": (
        "488ba0d5d7896c09a4966d90d16c831090f516c42ab213c70c62aead6b49f0b5"
    ),
    "runtime_deferred_lifecycle_validate_exact": (
        "63e7de646d2eb0dcbc7c9e8aaa87562e44ee3bb007b3185e51237b0b554e4cb3"
    ),
    "runtime_deferred_lifecycle_validate_against_ingress": (
        "5584747a03531f1709f299d16b7cd1884becd7cbd5a37c7619777277112b4e30"
    ),
    "runtime_deferred_lifecycle_validate_active_against_ingress": (
        "0ae0c8bde63fd29b4732f301498515f9444a9991f8008aaf3fc23a9a27583918"
    ),
    "runtime_deferred_lifecycle_rebase_deferred_ingress": (
        "981bd3b563516a04eb013b82fbacc7017b4d3a8df16756bcf5c5d7cd72808de4"
    ),
    "reconcile_deferred_ingress_ownership": (
        "b59feb05b4740924791b9eaba5b76a4b6f20bc19cdc4b65febeaecaddcb6db9e"
    ),
    "reconcile_deferred_runtime_ownership_after_retirement": (
        "dd51598b026d3c5bc97abfcf934b04438ac3f57f1f7577fa6faac4c084953603"
    ),
    "active_leader_wire_runtime_ordinals": (
        "60659f35ec4d592990db68069fb1373bc2af662711fb11a14db1078d8b395dc6"
    ),
    "retire_orphaned_leader_wire_runtime_receipts": (
        "9c4e92b725c4da1cf83dcb82ae6f76b8d9906ec4fbcdca1117dfadef6024bd7d"
    ),
    "register_leader_wire_runtime_receipt": (
        "58838980b5f6eac5ad28b4e40af9bbfaab41e4d2a32e5215fbe918a890e6ec11"
    ),
    "complete_leader_wire_runtime_owner": (
        "e17e62beccb6e2e219f3aac01c126456531ffb0448930b83026d9cf02da6695c"
    ),
    "complete_driver_dispatch_leader_wire_owners": (
        "6fccf9497abffa385d06a6e4f4fd9a0f641745ca51b17f8b6e7dcede26c449eb"
    ),
    "accept_driver_dispatch": (
        "33e443dbffe23a764940bf525a0a65a2539a96af0616dd954ec0c9ce0906207b"
    ),
    # The public enqueue alias is shared with the timeout-episode inventory;
    # the two admission predicates retain independently reviewed seals.
    "enqueue_network_with_ingress_ownership": (
        _RUNTIME_ENQUEUE_NETWORK_WITH_INGRESS_OWNERSHIP_ITEM_SHA256
    ),
    "can_admit_pre_runtime_leader_wire": (
        "c5aea05f967c2fdaef5232e0fe04abb824047a221ed059115e8ad3a54620bdbb"
    ),
    "can_admit_network_message_with_ingress_ownership": (
        "338ddd65e84755b37a84a618375f27631ddabc72e478b2405ee7f9904b5ecad3"
    ),
    "take_last_scheduler_ownership": (
        "b781f7ace9823e4ba2b395230912a703a78c2b6ae8fb48e96a0f0f120c9fa7c8"
    ),
    "network_admission_uses_exact_normal_and_progress_reservations": (
        "4d2fa0f4659961711a292716ab926a996ce38758605afb61e6c4796641a79341"
    ),
}

_PRODUCTION_CAUSAL_FIFO_RUNTIME_REGRESSION_SHA256 = {
    "ordinary_step_skips_only_blocked_prepare_qcs_to_install_matching_tc": (
        "e3aad9aae8a82eb29d223230fa3f8b2cf26ba5dfa75b6859be40d0ce1cd45b71"
    ),
    "ordinary_step_skips_future_prepare_qc_to_install_ahead_tc": (
        "97f478fc656843988e4fc1323a9ed957e67b98be13444d7e4de326b985763a67"
    ),
    "ordinary_step_skips_future_prepare_qc_for_higher_view_commit_qc": (
        "74c014055d906b67924cec81d623a72c9ec9a1129b09941cad0c1b729b5dd9b3"
    ),
    "deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences": (
        "275408912512b0588a7fd1403629e9039115288b25d66aa707ddfb77a66d902e"
    ),
    "later_same_semantic_fair_retry_retains_runtime_lifecycle_root": (
        "12e41e90bcd59ef22389131ad4b5cdeb56dce7fa04cd15357d18ce9f62cb1b6a"
    ),
    "older_frozen_aggregate_carrier_rebases_queued_runtime_minimum": (
        "95df3889b70c9358ed887e6bdf4c5ad6a08e361d10d8415e8587057e765d8d38"
    ),
    "network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals": (
        "878269e9bdf567147360a1f3c2bf5a7def18b7abd1b38aed3978dc7923624291"
    ),
    "distinct_pre_runtime_leader_wire_qc_waits_behind_busy_deferred_owner": (
        "70e6ec9072aa3675905f837b2aace19b2a985f1643bb9f05796689aa2979a708"
    ),
    "pacemaker_escape_coalesces_prequeued_distinct_origin_prepare_qc_into_live_busy_producer": (
        "5ae5c533c0498b9d067634d88722b736f52ff78c6a63983111a390c36c3603fe"
    ),
    # This alias is shared with the timeout-episode regression inventory.
    "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner": (
        _RUNTIME_RESTORED_PRE_RUNTIME_TC_CANNOT_DEADLOCK_ITEM_SHA256
    ),
}

# Exact comment/literal-free token seals for the finite current-view
# TimeoutVote recovery episode. Each entry is approved only after its focused
# positive baseline and negative mutations pass together; no digest in this
# inventory may be bulk-refreshed.
_TIMEOUT_VOTE_EPISODE_RUST_ITEM_SHA256 = {
    "ingress::fair_v2_ingress_queue_gate_verdict": (
        "217a9045fe04898fdbe2190451c990e44f75fb54b07f73197fb88a755a0cf75d"
    ),
    "ingress::select_fair_v2_ingress_candidate": (
        "9d12522aa0b65a229efc08e35feae5d887c7656366fa074e05b14c2c370a6068"
    ),
    "ingress::try_recv_if_checked": (
        "091f57ccb6adaafd50864565891f636b364658cb8ed70cc5254d521901779a82"
    ),
    "ingress::try_recv_if_checked_retiring_obsolete": (
        "005b9b5d1759840f0b68cc2b933c124309a69ac0522dc95ccd3a63ce33c25aa5"
    ),
    "ingress::try_recv_if_at_checked": (
        "73722eaedc36f6ef5265f77198fb95ea520b686ea71406cca9326a8376c2c13b"
    ),
    "ingress::try_recv_if_at_checked_classified": (
        "ca657eaedc48fdfdf96aeca1558d4b17774762c22fd0cd7e8cce82270aff5487"
    ),
    "runtime::RuntimeTimeoutVoteEpisodeOwner::validate_against": (
        "fb2a98e36b7014a4199ae78eda34119f69cec26fc25fff2d4ce5758f41d74777"
    ),
    "runtime::RuntimeTimeoutVoteEpisodeOwner::same_lifecycle_owner_as": (
        "4c7e987d86308f3f71c36a9c91f9f4dfdda750f411382dd5934fbb61554668bb"
    ),
    "runtime::RuntimeTimeoutVoteEpisodeAdmissionPlan::count_transition": (
        "63923a863bfa409746d6f5efe7a5609913a9ebd19ffe98d65281a2e9a94f310f"
    ),
    "runtime::emitted_timeout_recovery_owner": (
        "62364351b1c0cbc70ec583b61313e53a374b60fc83e4b4257beefcd35c953fa5"
    ),
    "runtime::timeout_recovery_episode_allows_clock_blockers": (
        "4c52b5a78486a535d247919079bb60090ace9ba4b77e1759763aec0f48627f55"
    ),
    "runtime::timeout_vote_recovery_candidate_from_fair": (
        "8a3399864c9016a838a2c64df3ac10e1642e9acdf9275e7b054c0160e7905072"
    ),
    "runtime::timeout_vote_recovery_candidate_from_runtime": (
        "edbe98f16b7a80cb790a6eee1c0a3026c7e6030e005aeaf0d4ef81de45403594"
    ),
    "runtime::timeout_vote_origin_binding": (
        "fd073c0810f106c086aa125073555bd4ebd9c2b09b214dd13b3a0c0515b44088"
    ),
    "runtime::timeout_vote_recovery_candidate": (
        "9ae72ebf117301b85a5dc61e4b2d2ec7a881484fbf387fc9a813f11f136766e5"
    ),
    "runtime::timeout_vote_episode_admission_plan": (
        "a4e4b2455b2c355c57bdeab05470e4a700afa498980adb02ed31504f75ed01ec"
    ),
    "runtime::enqueue_network_with_ingress_ownership": (
        _RUNTIME_ENQUEUE_NETWORK_WITH_INGRESS_OWNERSHIP_ITEM_SHA256
    ),
    "runtime::can_admit_timeout_vote_recovery_episode": (
        "9b99f56691a9752e95501583a1e187d8727498035d8d899c55277ccd3ffde47b"
    ),
}

_TIMEOUT_VOTE_EPISODE_RUNTIME_REGRESSION_SHA256 = {
    "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner": (
        _RUNTIME_RESTORED_PRE_RUNTIME_TC_CANNOT_DEADLOCK_ITEM_SHA256
    ),
    "restored_pre_runtime_timeout_vote_releases_only_an_absolute_timeout_cut": (
        "00eb6a4425997fe9ef985adb09e792913c5df8805e58c4de9473e7fd720724d2"
    ),
    "pre_timeout_scheduler_owner_may_publish_across_the_physical_snapshot": (
        "c1bbb1ecbce90f807f45eb40edfc834fa18d636824b47abc9364c7f4571442ef"
    ),
    "two_fresh_timeout_vote_slots_replenish_once_and_close_a_four_validator_view": (
        "3627cc149c725afbfc064f283eecf45af650ef9fa3345afee130d5484c0b1620"
    ),
    "restored_timeout_vote_reactivation_binds_fresh_carrier_before_runtime_admission": (
        "88bd126133d6471dbd53c474136ef99b5fe7a5816b488288f6e2939516d63d68"
    ),
}

_TIMEOUT_VOTE_EPISODE_WORKER_REGRESSION_SHA256 = {}

# Comment-normalized formal operators for the same finite producer episode.
# These remain fail-closed placeholders until every listed semantic mutation
# is observed to fail independently.
_TIMEOUT_VOTE_EPISODE_TLA_OPERATOR_SHA256 = {
    "SumeragiV2AsyncNetwork.tla": {
        "AsyncLeaderWireCanonicalLifecyclePayload": (
            "b921a227ed962459b4973297dd0d9be90c81401af62ba7f385e398de31a8b453"
        ),
        "AsyncLeaderWireLifecycleSubject": (
            "77c927fa76dda21438c34668681704a702894bde0e6497f57c3518c9f4acb1cb"
        ),
        "AsyncLeaderWireLifecycleIdentityAt": (
            "3144055cc1d2ba15799bc2f716423a113d1998fcdfe2e1e5410e437aafac0045"
        ),
        "AsyncLeaderWireLifecycleRecord": (
            "854dd230bb474215e7b417f3d80d3327811b5aaf3a1eb5cb2ca4c3c3bdb6bc51"
        ),
        "AsyncLeaderWireLifecycleTyped": (
            "dc59143994e349417932182b8042555bd572c60cfa7a6ccf37dd3db9d2252053"
        ),
        "AsyncTimeoutRecoveryVoteOwnerDispositions": (
            "f6d0a37b228934a0f88c3d9ae8cc46aec9ed1331ae9172b5784ef78ec6092090"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionDispositions": (
            "2c2ef87daca3055b2b0747d1664ad617a826c85370434509f8ea58b6050c4c23"
        ),
        "AsyncTimeoutRecoveryEpisodeKey": (
            "61dfcd9bf069add7d70901f79ac047bf57fad89e302b3855c9dddc3ea390da59"
        ),
        "AsyncTimeoutRecoveryEpisodeKeySet": (
            "35f54e3863b1adea21d2f768a5be5862d13d9c4aaff3e41c9eb54fbb3db3a37a"
        ),
        "AsyncTimeoutRecoveryEpisodeParameterSet": (
            "97e276e0aa217f74f2670a2bd2102ff27100a460d14fcd07af09e96d201de2e6"
        ),
        "AsyncTimeoutRecoveryEpisodeFromParameters": (
            "e545c3e4954c93475837024ba05441adbd7e847c72af1ef7293099a2c40603f4"
        ),
        "AsyncTimeoutRecoveryEpisodeSet": (
            "25949f35225bb00891c0ce09e0f52a9b73677ef92d0336b5337c9d1d11966fbd"
        ),
        "AsyncTimeoutRecoveryVoteOwnerSlot": (
            "681ac1b76eedfd4cf39b4407c5f75a199fc3088fcb89ff2704ef0450e32e757f"
        ),
        "AsyncTimeoutRecoveryVoteOwnerUniverse": (
            "eb5c4dc611f078a882b491ffba0e33446987966a124e23630b9835d214e5cdb5"
        ),
        "AsyncTimeoutRecoveryVoteOwnerValidForEpisode": (
            "5a7bdb2dff26729c0a73dd25a76f084af512fd78152d84511d77f0cf48b9faa0"
        ),
        "AsyncTimeoutRecoveryEpisodeValidIn": (
            "ebc240a8012b97d2d93de0c8e89b8dc59d16555b4b6246bc5f80640e974aa53d"
        ),
        "AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant": (
            "67dda9f1b2cf4edf41d7ef7a7e7279615e9472dc97895e9e677208b882560879"
        ),
        "AsyncTimeoutRecoveryVoteBasicBinding": (
            "7bb504333f577e6112b7d4e700c5bd06273c6a991f37f19b5a2ad148bd3fe286"
        ),
        "AsyncTimeoutRecoveryVoteIngressRecordsFor": (
            "bfa3c52d46c7f667b1c4692190c73202864dde163cb07948424677b7336c0ebf"
        ),
        "AsyncTimeoutRecoveryVotePreCutDescent": (
            "e281c02fe826ce29730d14d8d1660a1fe850ef6e37ba9957d6239e494d8278fb"
        ),
        "AsyncTimeoutRecoveryVoteRestoredDescent": (
            "b62e3824511a155faa4a16e5b264675d112b41c47705d634dc0da866880e1db1"
        ),
        "AsyncTimeoutRecoveryVoteFreshReplenishment": (
            "866f9a2e43a1e7434faa9ed0aabfa15e5a880e8d34b50812c39331763df2bed5"
        ),
        "AsyncTimeoutRecoveryVoteDispositionMatches": (
            "7d7b112535e7390fcc77ea79d994ece09822223bc2f58c255dd57732dd7437f9"
        ),
        "AsyncTimeoutRecoveryVoteCandidateOwners": (
            "30123fde668ec8ff7f8142ba8eb692926e42c9a4bdbb17a568ac3cbd940d1173"
        ),
        "AsyncTimeoutRecoveryVoteCandidateDefined": (
            "0050d45aafb4106ebc997e8b044c310806bbfdaf5233b712374a0a6d3e9889a6"
        ),
        "AsyncTimeoutRecoveryVoteCandidateOwner": (
            "f7fdd337e582dc62d429eab16fd16674b17e07890d2753301f66d9ae0c91bc66"
        ),
        "AsyncTimeoutRecoveryVoteIncumbents": (
            "5de335606d8bf7816ddfea5b85dd1716545d0ca267bd22e89e9888bb732350a1"
        ),
        "AsyncTimeoutRecoverySameVoteLifecycleOwner": (
            "7c638dd0530b7bd889e0755a45eda303f98359575397a219e86593067a78caff"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionPlan": (
            "d0da8ba07c67af4fd18a668523e3748f3972a66f85b05c2b12795ece44068b55"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionRequired": (
            "2f5ee1a824beb7736d0bbb7f0c051b29fee24fb4a782c56a3a7b1ab863afc273"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionAllowed": (
            "8dd0ff9ee7a45c7bd12d8cee1c0ba3c155115b1ec6f8cc83bbdfc237fe14b7d9"
        ),
        "AsyncTimeoutControlDependencyAdvancesLeaderWire": (
            "f823bff8bf0319f9bdea8a9286328f81e850303b2ca7188655ea990ff50fa143"
        ),
        "AsyncServeIngressIndexMayPrecedeAdmittedTarget": (
            "836fb332ad28f0f0a5a5afa54ac9e14d1c0e74200427327d2a5c9dc2f328887e"
        ),
        "AsyncLeaderWireIngressIndexMayPrecedeAdmittedTarget": (
            "56bfdf30d6624ac2f8561e250c3072cb350123e4a2742effa0ef8068b65410eb"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionOccursThisStep": (
            "28f5c1e16034f5dd5c3cae9ccf22c81cb02e1d9ffc1eb20aa04f04e6b915b385"
        ),
        "AsyncTimeoutRecoveryVoteRuntimeRecordsAfter": (
            "48aa5a6e4b6422d73214ea872f4fa817071bd17b2a6fa4c55b59aa100f0ac5b2"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionNodesThisStep": (
            "cc519f7ac25e4fed87ac00a68d907e97b72b1385eb6d527d4ed94facde6456a8"
        ),
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmission": (
            "f49556faa311370cee18c10d8af8489054c87bfceca95df7e10ca49f4114e0b2"
        ),
        "AsyncTimeoutRecoveryVoteOwnerStateAfterAdmission": (
            "37d807e2f76d7170ca7145829e8ebe8b2fafd5f2b24ea0130e3beff0fa5d6021"
        ),
        "AsyncTimeoutRecoveryPreAdmissionStateForSlotTransition": (
            "acf7fbae28f75e45ecd317451bc2e38ab865fab16acdc736d0bd8024d952d200"
        ),
        "AsyncTimeoutRecoveryAdmittedVoteSlots": (
            "b09ad8beca8d2938814d9d19fd3446c596f01131e885bbb4f8875f1749800183"
        ),
        "AsyncTimeoutRecoveryRemainingProducerSlots": (
            "a3c7c870d7110bfcf64c6be0e5889f13fb836379b55954775349f11ab762a75c"
        ),
        "AsyncTimeoutRecoveryProducerEpisodeMeasure": (
            "316131577d432419223b8e039fc85c66144da1996701736cf6a04882105747b7"
        ),
        "AsyncTimeoutRecoveryEpisodeCreationReadyIn": (
            "463fcd6b8a67c383cd62b258d220831cd60d0854df625b77b359ff3859827283"
        ),
        "AsyncTimeoutRecoveryTransitionGateIn": (
            "02e689cac6c1aba321ea319591aff3feb1957c946608e9cc86f3826c854445ed"
        ),
        "AsyncTimeoutRecoveryEpisodeRetiresThisStep": (
            "ec514a038cee6fb9f31e63409040bee22defb732edc7208e3a683779a24ad686"
        ),
        "AsyncTimeoutRecoveryExistingCaptureClearsThisStep": (
            "92a31d4bd2a3c6b1a57c6bbbcf5d67d33da9b575922159aa12074e373226eb1d"
        ),
        "AsyncTimeoutRecoveryEpisodeAfterTransition": (
            "043fa928e89e959c987aa19209e2c3ef68541a551017062b9f38ae1a0d267fbc"
        ),
        "AsyncTimeoutRecoveryRetainedEpisodesAfterTransition": (
            "15b01a6485f2446a013ff833d7ed3831dca42ba6933642afa74bb123622d7f76"
        ),
        "AsyncTimeoutRecoveryNewEpisodeIn": (
            "dfb0f545e1804325d054d841731c71e8da09565a9ae99177980e66b42f828b54"
        ),
        "AsyncTimeoutRecoveryNewEpisodesAfterTransition": (
            "0bb56383a4306a31b7df629c7908fd37f772a5c31305ee641b887c287a46b3f2"
        ),
    },
    "SumeragiV2AsyncRecoveryVoteEpochProofs.tla": {
        "AsyncTimeoutRecoveryEpisodeBoundaryIn": (
            "a45ca5da201b0d81c538304cfbd4740029c9b8aaa17190f2c0f835a99ec5bc8a"
        ),
        "AsyncTimeoutRecoveryBoundaryFrameShape": (
            "7b851fbaf17ebee465208fbde0c47b447c6cf9e278cc54b369ce00da2146eecf"
        ),
        "AsyncTimeoutRecoveryMutationFrameShape": (
            "f85b7549b33097ba1cf31adbb63d48a85fdf0b61f409b60e128451acbf37656f"
        ),
        "AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor": (
            "5bb5d4962bbc9db7fcbe00c53e70d5532ed39550ee5da04acdac6048dc0fc5c3"
        ),
    },
    "SumeragiV2AsyncRecoveryVoteEpochBoundaryContinuationProofs.tla": {
        "AsyncTimeoutRecoveryNewBaseEpisodeIn": (
            "aebba5c89eb28cc959098eef0a148c0a3defe3818d71f7aacc07ab8f2fd73136"
        ),
        "AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn": (
            "e90869e129c22b6a56cdac3759bcf47be53b7c89276675019eeb1b7e39891d2b"
        ),
    },
}

# Exact theorem statements and proofs which close the finite producer episode.
# These seals stay deliberately unapproved until every independent theorem and
# downstream-provider mutation fails after its local digest is refreshed.
_TIMEOUT_VOTE_EPISODE_TLA_THEOREM_SHA256 = {
    "SumeragiV2AsyncNetwork.tla": {
        "AsyncOrdinarySelectorPreservesCertifiedResponseBeforeTimeoutVote": (
            "03f998f79bfcc6c1fedad6a3b984f0404e5c45628430a7497575835fca42c7eb"
        ),
        "AsyncTimeoutRecoveryProducerEpisodeMeasureIsFinite": (
            "1df06952a3664f7770385d6d4175e73bfbeddc53200715e3ccd627ac925a39f9"
        ),
        "AsyncTimeoutRecoveryFreshOwnerRemovesExactlyItsRemainingSlot": (
            "88bd8b5a5262c9e4220a14c1041492fea1435795035c0df5833da6b01ae7b8f6"
        ),
        "AsyncTimeoutRecoveryNonCandidateCreatesNoAdmission": (
            "09724aa3803ff9a87bbc61b24f101afa4d2ead895f167a3c05beb3bd59403698"
        ),
        "AsyncTimeoutRecoveryFirstAdmissionConsumesExactlyOneProducerSlot": (
            "44874f718618e4409812b99ff62e28cefa9dc904a4938889e04104a5a4f1485b"
        ),
        "AsyncTimeoutRecoveryCoalescedRetryPreservesProducerEpisode": (
            "4c25562e7e3e5ba4b301a083caf80fc507ca23793fdff5f0d83a61d98ab598db"
        ),
        "AsyncTimeoutRecoveryFreshReplenishmentConsumesFiniteProducerSlot": (
            "402d555a2123de59812a33b3beb9efd8018fa08f70d293b34284ea83d12ebd12"
        ),
        "AsyncTimeoutRecoveryUpdatedEpisodeIsRetainedByAdmissionState": (
            "81b6e5c8ab71af82a8099a563a96db2e5ee106fc31c9d773b86db2c7652c064e"
        ),
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionIsStateIndependent": (
            "f23a0f081c1b20572022d0756f8daa9ee1dbc06da96b1e4ed933fe2406b53686"
        ),
        "AsyncControlServiceTransitionRequiresAtomicLifecycleReservation": (
            "4ff696a18c898ff16ba41a705e464668de9083c018efdafe1baccbeceeae3cb7"
        ),
        "AsyncTimeoutRecoveryRetainedEpisodesContainFramedEpisode": (
            "21fad6c95f6d72febeaea68a67b42c917c44c5d3958203eb2fc2e442227c1529"
        ),
        "AsyncTimeoutVoteFairIngressDrainLeavesCoreState": (
            "579691707af9891234bdee0b535a018dd864baac48548c17ddac66f428dec26e"
        ),
        "AsyncPostGstHasNoControlServiceReset": (
            "d738c3b56078f2a44f76f15c319d310288f9fad7a2ec011a7cdc04af5d06e36d"
        ),
        "AsyncTimeoutRecoveryEpisodeCurrentBoundaryForNode": (
            "d3cd30a1100860fd8c718fce20f20379aba20ed13974fa7eed912503f9138ce8"
        ),
        "AsyncUnchangedCoreStatePreservesTimeoutBoundary": (
            "47d944b51bd1a575c066d1d3218e62de883b07984b04725ae9355234f73a20cc"
        ),
        "AsyncTimeoutVoteIngressDrainRetainsCurrentEpisodeBoundary": (
            "2ea11aa8dc93fa7684d2ecb2a96cb4af7dbbbc8da63d30a008048a2a1fd045aa"
        ),
        "AsyncUnchangedCoreStateExcludesPersistInstall": (
            "095b9ab62786836bcfbb5a4363477378905311a89aebc4d1fcf4feb4bd7ba97f"
        ),
        "AsyncFairIngressDrainPreservesRetransmitTimerState": (
            "b473620a788d1b0f680dc34677d31471a8f218415ab2572c2d18835c1e64b2d9"
        ),
        "AsyncFairIngressDrainExcludesDirectRetransmit": (
            "ad31fdd4ae63ba80ec7d6ca8e9944f5dcc0f8c237ceb6596b35d23d12f7ea674"
        ),
        "AsyncTypedOutstandingTagRemovalChangesFunction": (
            "8ea219351421de1566369e590290bebafc6fd19d80ef1f3173b81d9cafdc11de"
        ),
        "AsyncDeferredRetransmitRemovesOutstandingTag": (
            "4b85c87bc746d4fee0e6844c5f1f3cb72c53a9acaf3cdb8b0f15a2fd69744bb0"
        ),
        "AsyncFairIngressDrainExcludesDeferredRetransmit": (
            "bc19fc379f8da8f848667b77cb0c859605fae3153e2ba6d2a2be334f0b40c17b"
        ),
        "AsyncIngressDrainDoesNotCompleteRetransmitLifecycle": (
            "21b37b8848a7bd3922bf934c2ae2533af315f563465cd07866562762a82dd959"
        ),
        "AsyncIngressDrainFramesDeferredAndCausalQueues": (
            "ed85c48c72dd007237193ba7a5087ba3f635b4fb2c5d9e3c05f5ac3fd78ceb59"
        ),
        "AsyncTimeoutVoteFairIngressFramesCommandAndWork": (
            "9ec8a9f5753e50c531e67e7ff18140a6a442f82afee336a0da542aa24f370c9c"
        ),
        "AsyncSequenceSetAfterAppendAddsOnlyValue": (
            "54303c304b4d75584ef078930e256dbc611662ba19d72a80907c555c4d328ee2"
        ),
        "AsyncUnionOfSequenceSetsAfterAppendAtAnyKeyAddsOnlyValue": (
            "41fef605e80b132b9fa9dc1e8be04bbcdc05b95cd3ecb7816db3866d2bcdf01e"
        ),
        "AsyncTimeoutVoteIngressDrainFramesSchedulerCarriers": (
            "333bf376ecd4a6de1510f3ef6be559e847804cfc5b5c99abe06bff76a27e241b"
        ),
        "AsyncTimeoutVoteIngressDrainAddsOnlyDeliveryOrigin": (
            "ec3bf0910e32f4ff43e9c2de072985cfce9ee193894b49857cba30fb2a95222c"
        ),
        "AsyncProposedTimeoutCausalOriginHasBeginTimeoutPhase": (
            "9f877ab73f3e7dd856d0e815adcd53f49537531203273d688470bee4af9cf8bf"
        ),
        "AsyncOwnedTimeoutLifecycleOriginHasBeginTimeoutPhase": (
            "9e91b44806f1f022f7b864cf76c6195a7e4a7d4e47e47685f5cb8db75e426053"
        ),
        "AsyncCurrentTimeoutCausalOriginUsesEffectiveOrigin": (
            "7895a9a9334907f80dafa4f489440db25dd1834c76d1aec5aad4fbf6b3343ae2"
        ),
        "AsyncOwnedTimeoutRecoveryCurrentOriginHasBeginTimeoutPhase": (
            "24f1cc906957182fcb9cc2070f04a36d3d412c649f7eb3266efaac4af6fc3774"
        ),
        "AsyncDeliveryCandidateOriginPhaseEqualsDeliveryKind": (
            "6738b6c3ff4b2f59e8a76c0f3d48a45a1596f33c1034e63bbccee12f687aef9b"
        ),
        "AsyncTimeoutVoteDeliveryOriginHasDistinctPhase": (
            "8575237ea078b3f9aa6ae52221c7005926d933edd1f64e6f76d105ee10b1bde1"
        ),
        "AsyncTimeoutVoteIngressDrainDoesNotTransferTimeoutLifecycle": (
            "ef4c3d237655ff50311beb25ccd505c64a08ec4d7101dc8cfb8561eb27a442ad"
        ),
        "AsyncTimeoutVoteIngressDrainEstablishesRecoveryFrame": (
            "f042e10c01d4109284f2557ce3da9806f771009526a3ea528b8d899ad794cb64"
        ),
        "AsyncTimeoutVoteFairIngressDrainFramesRecoveryEpisode": (
            "43b470ee0007250242614c689a697ece3ba3c5b97a508ae5dd184f12d9431c28"
        ),
        "AsyncControlServiceSlotTransitionPublishesTimeoutRecoveryVoteState": (
            "47d20f52eb8495d81169989c68ba754f7f36bfa6622b02a56433240f41367c54"
        ),
        "AsyncTimeoutRecoveryVoteAdmissionRetainsUpdatedEpisodeAcrossSlotTransition": (
            "9b7852d4c48d9c782e1e06ad51abdf998b9fe770e65a4c656a13231ce080f93f"
        ),
    },
    "SumeragiV2AsyncRecoveryVoteEpochProofs.tla": {
        "AsyncNetworkStepPreservesEmptyServeIngressOwnersWhileProducerTurnReady": (
            "2b82a05a24eabc175cc34bb7dfa19d2a5f9d1ee2fa745169c3ea13de8d1f682d"
        ),
        "AsyncNextPreservesEmptyServeIngressOwnersWhileProducerTurnReady": (
            "4759d2f074d0c77694fcd0aec61d6c72fc9f2f68b87a9013c321203a08d6cc71"
        ),
        "AsyncNextPreservesServeProducerTurnTypeInvariant": (
            "be2bca26611e684353c591890ee166a9ec007cdec385d25018cb0331cfeee846"
        ),
        "AsyncNextPreservesServeProducerTurnInvariants": (
            "5ceb7abda7dceaaaaa54ed2b174385a25af5412f8574563c6cedcc3619506421"
        ),
        "AsyncTimeoutRecoveryMutationFrameProjectsBoundaryFrame": (
            "1b4c3dfb92c959259a888b735f68844adab19402ea9fb2898dbddea784965c8a"
        ),
        "AsyncTimeoutRecoveryEpisodeFromParametersHasMutationFrameShape": (
            "4d2c2d79ef909af71925238d588ed7c64d76a589f2792b699ea1adc63ede7a1f"
        ),
        "AsyncTimeoutRecoveryEpisodeSetHasMutationFrameShape": (
            "919a0a8655b7d59f9ff637e654c32dbacaa5e3111f06495a757fd9b4a5926050"
        ),
        "AsyncTimeoutRecoveryClearedFrozenPredecessorPreservesCurrentBoundary": (
            "d83c752c87732813a8c874cbc6767d8e094b8babe60b5fb9d2c7a03055b12ed6"
        ),
    },
    "SumeragiV2AsyncRecoveryVoteEpochBoundaryContinuationProofs.tla": {
        "AsyncTimeoutRecoveryEpisodeAfterTransitionPreservesCurrentBoundary": (
            "8dcef1fda3370f5227fd39b43c1361a7607e16f41e7ccce7d8c2fae1220a96a7"
        ),
        "AsyncTimeoutRecoveryRetainedEpisodesHaveCurrentBoundary": (
            "165cbd893a1cea61c0c91cfd04ce9d7f955bd66ff0ec6c342fcd1668a4b04429"
        ),
        "AsyncTimeoutRecoveryNewEpisodeDecomposition": (
            "b5ee625013c0ceb306bc595c023bd508605aa53015dc870417f70c046f8532ff"
        ),
        "AsyncTimeoutRecoveryNewBaseEpisodeInHasCurrentBoundary": (
            "7904b774302b1d694602a1998453a7eda81b24476c907a6f0ad9db82a4e062a4"
        ),
        "AsyncTimeoutRecoveryNewEpisodeInHasCurrentBoundary": (
            "e2805c35555463b3deb60c7406ca6857bbf8be56c176e865e64417085519a43d"
        ),
        "AsyncTimeoutRecoveryNewEpisodesHaveCurrentBoundary": (
            "ff78a382b38ce79b326523adc660a8b44be48edfdccb4cce8125f9cea9319cd8"
        ),
        "AsyncTimeoutRecoveryEpisodeUnionEstablishesCurrentBoundary": (
            "9729dc41b99b9e312a7720b1d13eba71eae422822ff341a3d22b3dddf9c8146f"
        ),
        "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionPreservesCurrentBoundary": (
            "5c0bf320cc8859bf01595208361885d31718a1b6cf99fcad84a0104de0927cbf"
        ),
        "AsyncTimeoutRecoveryVoteOwnerImagePreservesCurrentBoundary": (
            "2373ca20ddef74981b6efe3ca4d917b3f305aed0a6eda616559c7ab770954b9f"
        ),
        "AsyncControlServiceTypeProjectsTimeoutRecoveryEpisodeSet": (
            "63b50294ef186208a2a221563512cf3954df54e9325cec390b70c782af1b189c"
        ),
        "AsyncControlServiceResetPreservesTimeoutRecoveryEpisodeSet": (
            "3ae8217b29fadcf0f9bd24bf30e60d03c4951f3c51bc89a536455201430adfa2"
        ),
        "AsyncControlServiceSlotTransitionEstablishesTimeoutRecoveryCurrentBoundary": (
            "cd37abed710d6c5fda8727bc5131ffd5127e4ce6e8ff057d590e58973a978b41"
        ),
        "AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant": (
            "e7333ca33c4b997ae139c209748ae603fac1b24002c319dab295bcb02325ae4a"
        ),
    },
    "SumeragiV2AdequateLeaderServiceClosureProofs.tla": {
        "AdequateLeaderFreshTimeoutVoteReplenishmentConsumesProducerSlotAndOpensNonDescentEpisode": (
            "3cb5653b8cceb88b3a193570b5b344cde20f58e5759bfb770fa343f9e392126c"
        ),
    },
}

# Exact production effect-executor entry points which retain the authenticated
# ingress carrier and latch a fail-stop restart on malformed ownership.
_EFFECT_CAPACITY_PRODUCTION_RUST_ITEM_SHA256 = {
    "enqueue_network_with_ingress_ownership": (
        "9bf3e8ec45247e920681de7f13941d8544df863b73c5a3ac243adcacae0b2587"
    ),
    "can_admit_network_message_with_ingress_ownership": (
        "b2d35847177381a3f4ab5fd6fb06ce4b15e7449220db5daac00699cc5825677d"
    ),
}

# The retained exact Fetch lifecycle must not duplicate service work while its
# immutable owner is already installed. Completion must publish the reserved
# runtime successor before retiring local ownership; protected-view transfer
# must move an unpublished token with its Fetch consumer; every terminal Fetch
# path must retire either that token or its exact restart-restored stage-7
# parent before releasing P/Q ownership. The checker separately binds the
# persist-first deferred batch, restart-only producer frontier, persistent-root
# rebind, and live leader-wire recovery cut by ordered source contracts. Bind
# the complete production methods and adversarial regressions in addition to
# focused order and coordinate checks.
_EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256 = {
    "begin_fetch": (
        "57871eeb94b6c37f635a5e605e559fe9d46a6c0f8e3e8f2c7dee3732ac3a2f08"
    ),
    "reserve_body_available_with_owner": (
        "a29bf418476f2b07e4821d63c6c1415ac611fc52a59868ea5fb7dc077d17dfa2"
    ),
    "rebind_unpublished_body_available": (
        "00bbe44f9d02199bee50f805425df0760caffc257598003f55aaa35ddb47d46c"
    ),
    "retire_unpublished_body_available": (
        "c692816b54ef0184f6453614a2c8272168ac445e5e1f397ab27a6b508354c6fd"
    ),
    "effect_runtime_rebind_unpublished_body_available": (
        "e126b97f8f5a75b0868f6334ada164ec16e2d35b924e6940daa91f5b5fe3550e"
    ),
    "effect_runtime_retire_unpublished_body_available": (
        "3ed15174c27d497028b91f264a25ec2c1294c35e228298ebc5caa51f74332df5"
    ),
    "commit_pending_fetch_retirement": (
        "be3ae121e9728b1bf3e49e1f290e08f8ed9f09c310e5e510adf7b5aac7e792a9"
    ),
    "install_view": (
        "09d31e09f4aa305f555b38165e6a27646096c9e2f858c06d3e43191f8d5bbdc8"
    ),
    "commit_fetch_completion": (
        "872cb0844dd0e1eb96ab8122b75f5cf9d4cc837f15581f07f81e02ea516fcab0"
    ),
    "retain_retransmit_effect_ownership_for_test": (
        "7501414a5b20cfeacf429a9d2b43cf5dd2a797e2a579559c05d987437352ed8d"
    ),
    "production_capacity_saturation_admits_response_and_reconstructible_fetch": (
        "934cac43859cca5ef5975f978a32934d88fd586b3b50fb97df1d62038e36e2c8"
    ),
    "unpublished_body_token_rebinds_retries_and_retires_as_one_exact_owner": (
        "920f70f6d4ebf90ce3f9a4365e99bc88da19671d80e4f716aa57bc08caece7fd"
    ),
    "tc_retires_unprotected_retryable_body_token_before_the_next_fetch": (
        "e7532e373c1e198bb4e4e8f9311e2adadeb61e1c9eff42a8d54175e1d1eabe83"
    ),
    "durable_sign_preemption_retires_a_retryable_body_token": (
        "28a96a094097d20132aa8c44f4a02fea3170fa544fbf84e7c499aa0b1b3a7789"
    ),
}

# Exact adversarial integration witnesses for the durable locked-Commit owner
# alternatives.  These tests are release inventory, not deductive evidence;
# whole-item seals prevent a renamed or weakened lookalike from satisfying the
# progress-witness regression contract.
_LOCKED_COMMIT_PROGRESS_WITNESS_TEST_SHA256 = {
    "locked_commit_progress_witness_rejects_inexact_or_empty_ownership": (
        "a40965bfa911b0f8b2cf118644aaf07d4cd6898246b1d5b72b9ea4e15649a9d6"
    ),
    "locked_commit_progress_witness_accepts_each_exact_owner": (
        "07823a34cefc84c62880027df0898fd5524c5e1e7f9ec71e0ebc84d7998e45b7"
    ),
}
_LOCKED_COMMIT_PROGRESS_WITNESS_HELPER_SHA256 = {
    "locked_commit_has_exact_progress_witness": (
        "e63115adfa82478faa975238d72973dbe84791f45fb8c968732d62974d2c44f4"
    ),
    "validate_locked_commit_progress_witness": (
        "fe6f766850236b7f363fb828e4acaa810eb676876e800dcb3fcd9b88711084ae"
    ),
}

_PRODUCTION_LIVENESS_RELEASE_COUNT = 863
_PRODUCTION_LIVENESS_RELEASE_CORRIDOR_LEG_COUNT = 85
_PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256 = (
    "42509872b04f64962dc8edc09ca9f007bafffe402c4e0847255dc937a105888c"
)
_PRODUCTION_LIVENESS_INVENTORY_GUARD_SHA256 = (
    "8eb88b75df56deaf2ff6684425b864a09269b8bb014346c336e5fac515f50c74"
)
_SUMERAGI_V2_PACKAGE_LAYOUT_GUARD_SHA256 = (
    "e99da2c824b86930b76c741d2f7aa47ab16092c2f84e43550fb6362a36133268"
)
_SUMERAGI_V2_PACKAGE_LAYOUT_VERIFIER_SHA256 = (
    "42fc1fb789e115df9f54c230ee6bfc1e1c20504a904aa20f945b6369df6d7679"
)
_CLOSED_SIDECAR_PREFIX_HANDOFF_TEST_SHA256 = (
    "75019365bd62839da229b51671071af1b9165f4c08fc06d36be6bc2e4e14b893"
)
_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT = 518
_PRODUCTION_MULTILANE_G_UNIT_TSV_LINE_COUNT = 519
_PRODUCTION_MULTILANE_FOCUS_INVENTORY_SHA256 = (
    "3d2c93cab0528cb668d977642eecfe78e9c20378e887a0b7db4198ffd220eb29"
)
_PRODUCTION_MULTILANE_FOCUS_CONTRACTS = (
    (
        "required_multilane_core_focus_tests",
        "g-unit-iroha-core",
        "iroha_core",
    ),
    (
        "required_multilane_queue_journal_focus_tests",
        "g-unit-iroha-core-queue-journal",
        "iroha_core",
    ),
    (
        "required_multilane_config_lib_focus_tests",
        "g-unit-iroha-config-lib",
        "iroha_config",
    ),
    (
        "required_multilane_config_runtime_focus_tests",
        "g-unit-iroha-config-runtime",
        "iroha_config",
    ),
    (
        "required_multilane_config_fixtures_focus_tests",
        "g-unit-iroha-config-fixtures",
        "iroha_config",
    ),
    (
        "required_multilane_data_model_focus_tests",
        "g-unit-iroha-data-model",
        "iroha_data_model",
    ),
    (
        "required_multilane_torii_focus_tests",
        "g-unit-iroha-torii",
        "iroha_torii",
    ),
    (
        "required_multilane_torii_shared_focus_tests",
        "g-unit-iroha-torii-shared",
        "iroha_torii_shared",
    ),
    (
        "required_multilane_integration_lib_focus_tests",
        "g-unit-integration-tests",
        "integration_tests",
    ),
)
_GENESIS_HEADER_BINDING_TEST_SHA256 = (
    "8d847d27cdea09a87f5ee4ec940f60f9fa73fb85ca9a965d2a3fcac19eb3b41e"
)
_RESTART_VIEW_ZERO_DEADLINE_TEST_SHA256 = (
    "13c1cd988856a8c4ee4d20cfc176c4111352ba7262d07bb417de5a4056cf8b1f"
)
_SUCCESSOR_PARENT_BINDING_TEST_SHA256 = {
    "successor_core_context_preserves_the_parent_certificate_binding": (
        "79c2caea8dfd6f17885ff3d72253a41cb34db7a99d7976b52d5fdab45c0e9a89"
    ),
    "successor_context_requires_the_durable_cryptographic_parent": (
        "02e40e352915e44d88584b7c93e73872383365a7de6d865bc79faebfd096403d"
    ),
    "authentication_rejects_valid_commitment_conflicts_without_mutating_adapter": (
        "5ab55fdeb8281a185b76e14c57dce22af0ea00c05da8820169de1a08721c86e5"
    ),
}
_LATE_LANE_RECOVERY_TEST_SHA256 = (
    "604a95487484ce18054c37fa211a64e47ec6bf9f347075b0c71e2794963e860b"
)
_PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS = (
    ("production-kura-progress-durability", "kura::tests", 18),
    ("production-kura-lane-geometry", "kura::lane_geometry::tests", 8),
    ("production-lane-relay-exact-ownership", "nexus::lane_relay::tests", 4),
    (
        "production-authoritative-ingress",
        "sumeragi::authoritative_runtime_gate_tests",
        42,
    ),
    ("production-merge-sidecar", "merge_sidecar::tests", 118),
    ("production-state-governance-unlock-audit", "state::tests", 1),
    ("production-queue-replica-disposition", "queue::tests", 1),
    ("production-v2-core", "sumeragi::v2_core::tests", 38),
    ("production-v2-core-refinement", "sumeragi::v2_core::refinement::tests", 17),
    (
        "production-v2-core-wal",
        "sumeragi::v2_core::wal::byte_lifecycle_tests",
        1,
    ),
    (
        "production-v2-core-source-link",
        "sumeragi::v2_core::reducer::source_link_tests",
        8,
    ),
    (
        "production-v2-equivocation-evidence",
        "sumeragi::evidence::tests",
        1,
    ),
    (
        "production-v2-leader-wire-lifecycle-store",
        "sumeragi::serviced_candidate_store::tests",
        1,
    ),
    ("production-v2-adapter", "sumeragi::v2::tests", 49),
    ("production-v2-body-store", "sumeragi::v2_body_store::tests", 2),
    (
        "production-v2-certified-serve-payload-store",
        "sumeragi::v2_certified_serve_payload_store::tests",
        11,
    ),
    ("production-v2-block-sync", "sumeragi::v2_block_sync::tests", 3),
    ("production-v2-apply", "sumeragi::v2_apply::tests", 3),
    ("production-v2-effects", "sumeragi::v2_effects::tests", 66),
    ("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 63),
    ("production-v2-runtime", "sumeragi::v2_runtime::tests", 65),
    ("production-v2-transport", "sumeragi::v2_transport::tests", 1),
    ("production-v2-recovery", "sumeragi::v2_recovery::tests", 3),
    (
        "production-v2-lifecycle-recovery",
        "sumeragi::v2_lifecycle_recovery::tests",
        5,
    ),
    (
        "production-v2-lifecycle-coordinator",
        "sumeragi::v2_lifecycle_coordinator",
        42,
    ),
    ("production-v2-runner", "sumeragi::v2_runner::tests", 37),
    (
        "production-v2-lifecycle-height-driver",
        "sumeragi::v2_runner::lifecycle_height_driver::tests",
        2,
    ),
    ("production-v2-worker", "sumeragi::v2_worker::tests", 90),
    (
        "production-v2-watchdog",
        "sumeragi::status::v2_liveness_watchdog_tests",
        19,
    ),
    (
        "production-kagemusha-finality",
        "zk::kagemusha_finality::tests",
        1,
    ),
    (
        "production-data-model-v2-finality",
        "block::consensus_v2::finality::tests",
        1,
    ),
    (
        "production-data-model-offline-compact-qc",
        "offline::kagemusha_v4_topup_provenance_tests",
        1,
    ),
    (
        "production-data-model-v2-context-identity",
        "block::consensus_v2::tests",
        2,
    ),
    ("production-v2-integration-runner", "sumeragi_v2_runner", 4),
    ("production-p2p-peer-reliable-flush", "peer::run::tests", 11),
    (
        "production-p2p-shared-source-byte-geometry",
        "peer::shared_byte_budget_tests",
        8,
    ),
    ("production-p2p-network-reliable-actor", "network::tests", 83),
    (
        "production-p2p-source-memory-geometry",
        "network::inbound_source_memory_bound_tests",
        2,
    ),
    (
        "production-p2p-waiter-rank-geometry",
        "network::handle_update_tests",
        4,
    ),
    (
        "production-irohad-consensus-message-control",
        "consensus_message_control::tests",
        8,
    ),
    ("production-irohad-network-relay", "network_relay_tests", 5),
    ("production-irohad-authenticated-via", "tests::relay_fairness", 7),
    (
        "production-config-v2-exact-output-geometry",
        "parameters::actual::tests",
        2,
    ),
    (
        "production-config-v2-exact-output-root-parse",
        "parameters::user::duration_clamp_tests",
        5,
    ),
)
_PRODUCTION_LIVENESS_RELEASE_MODULES = tuple(
    module for _, module, _ in _PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS
)
_PRODUCTION_LIVENESS_NEW_REGRESSIONS = (
    "kura::tests::lifecycle_release_terminal_outcomes_are_exact_idempotent_and_ordered",
    "kura::tests::autonomous_view_state_latest_read_only_selects_crash_temp_without_mutation",
    "kura::tests::unfinalized_merge_carrier_tip_rebuilds_post_wsv_reservation_on_restart",
    "kura::tests::merge_application_receipt_makes_autonomous_auxiliary_persistence_terminal",
    "sumeragi::v2_lifecycle_recovery::tests::generation_takeover_runs_crash_recover_and_rehydrate_then_stutters",
    "sumeragi::v2_lifecycle_recovery::tests::every_lifecycle_recovery_cursor_cas_boundary_survives_restart",
    "sumeragi::v2_lifecycle_recovery::tests::prepared_bootstrap_and_crash_boundaries_resolve_only_their_durable_side",
    "sumeragi::v2_lifecycle_recovery::tests::empty_queue_reconciliation_returns_the_same_checked_receipt",
    "sumeragi::v2_lifecycle_recovery::tests::local_producer_recovery_requires_the_exact_current_queue_owner",
    "sumeragi::v2_lane_work::tests::production_adapter_stays_carrier_silent_until_exact_queue_activation",
    "sumeragi::v2_apply::tests::deferred_canonical_carrier_owned_and_absent_groups_complete_before_gate_publication",
    "sumeragi::v2_apply::tests::deferred_canonical_carrier_missing_after_queue_cleanup_keeps_startup_gate_closed",
    "sumeragi::v2_runner::tests::startup_reconciles_lifecycle_before_lane_work_activation",
    "sumeragi::v2_runner::tests::terminal_sweep_source_partitions_whole_units_before_any_mutation",
    "sumeragi::v2_runner::tests::local_producer_queue_custody_is_preflighted_before_cursor_mutation",
    "kura::tests::certified_lane_block_encoding_enforces_source_envelope",
    "nexus::lane_relay::tests::actor_backpressure_retains_exact_relay_and_fifo_ticket",
    "nexus::lane_relay::tests::blocked_relay_does_not_starve_a_responsive_relay",
    "nexus::lane_relay::tests::terminal_actor_failures_return_exact_relay_ownership",
    "nexus::lane_relay::tests::saturated_relay_owner_returns_sixty_fifth_without_actor_ticket",
    "sumeragi::authoritative_runtime_gate_tests::direct_envelopes_bind_both_identity_roles",
    "sumeragi::authoritative_runtime_gate_tests::atomic_lane_certificate_uses_the_shared_progress_owner",
    "sumeragi::authoritative_runtime_gate_tests::oversized_atomic_lane_certificate_is_returned_exactly",
    "sumeragi::authoritative_runtime_gate_tests::relayed_origin_churn_uses_one_via_lane_and_preserves_protocol_origin",
    "sumeragi::authoritative_runtime_gate_tests::authenticated_non_validator_source_cap_retries_third_source_until_one_lane_drains",
    "sumeragi::authoritative_runtime_gate_tests::roster_origin_relay_completion_has_authenticated_source_count_and_byte_owner",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_wire_index_keeps_authenticated_origins_distinct",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_reserves_same_source_transport_completion_behind_auxiliary_pressure",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_escape_survives_exact_same_source_saturation",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_serializes_distinct_timeout_certificates_per_source",
    "sumeragi::serviced_candidate_store::tests::leader_wire_gate_retains_independent_cross_origin_phase_and_chunk_slots",
    "sumeragi::v2_effects::tests::effect_dispatch_consumes_leader_wire_terminal_created_while_batch_drains",
    "sumeragi::v2_effects::tests::retained_live_retry_consumes_decision_retirement_terminal_same_cycle",
    "sumeragi::v2_effects::tests::retained_recovery_retry_consumes_decision_retirement_terminal_same_cycle",
    "sumeragi::v2_effects::tests::exact_candidate_retry_coalesces_under_the_incumbent_owner",
    "sumeragi::v2_effects::tests::fetch_owner_replacement_is_rejected_before_upgrade_refinement_or_request_work",
    "sumeragi::v2_effects::tests::adapter_effect_retry_policy_is_closed_over_all_eleven_effect_classes",
    "sumeragi::v2_runtime::tests::adapter_effect_binding_is_exact_route_neutral_and_three_bounded",
    "sumeragi::v2_runtime::tests::certified_body_pipeline_retains_statement_and_owner_across_stage_kinds",
    "sumeragi::v2_runtime::tests::body_pipeline_acquires_commit_authority_monotonically_under_one_owner",
    "sumeragi::v2_runtime::tests::pending_validate_projects_exact_prepare_commit_and_report_successors",
    "sumeragi::v2_runtime::tests::pending_validate_projects_only_the_exact_commit_authorized_apply_successor",
    "sumeragi::v2_runtime::tests::drained_internal_ignore_uses_exact_durable_tombstone_before_readmission",
    "sumeragi::v2_runtime::tests::queued_body_completion_coalesces_only_its_incumbent_owner",
    "sumeragi::v2_runtime::tests::stale_internal_callback_is_marker_free_and_malformed_callback_spends_no_ordinal",
    "sumeragi::v2_lifecycle_coordinator::tests::restart_seeds_high_water_and_rollover_preserves_it",
    "sumeragi::v2_lifecycle_coordinator::tests::producer_handoff_blocks_later_work_without_making_serve_a_global_barrier",
    "sumeragi::v2_runtime::tests::decision_retirement_releases_queued_leader_wire_runtime_owner",
    "sumeragi::v2_runtime::tests::lock_retirement_releases_busy_deferred_leader_wire_runtime_owner",
    "sumeragi::v2_runtime::tests::production_authenticated_preflight_is_never_semantic_only_coalesce",
    "sumeragi::v2_runtime::tests::semantic_only_authenticated_coalesce_fails_before_receipt_registration",
    "sumeragi::v2_runner::tests::fail_closed_authenticated_coalesce_releases_gate_and_suppresses_retry",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_coalesces_semantic_request_and_attaches_independent_routes",
    "sumeragi::authoritative_runtime_gate_tests::alternate_reply_route_attaches_before_authenticated_source_lane_cap",
    "sumeragi::authoritative_runtime_gate_tests::transport_reply_route_construction_is_fallible_and_target_bound",
    "consensus_message_control::tests::stale_duplicate_reordered_and_unknown_releases_are_atomic",
    "consensus_message_control::tests::hold_capacity_is_bounded_by_count_bytes_and_checked_arithmetic",
    "consensus_message_control::tests::drain_fence_holds_racing_chunks_fifo_until_atomic_cutover",
    "tests::relay_fairness::hold_release_preserves_exact_layered_ownership_until_recorded_terminal",
    "parameters::user::duration_clamp_tests::sumeragi_authenticated_non_validator_sources_must_fit_network_geometry",
    "parameters::user::duration_clamp_tests::sumeragi_authenticated_non_validator_sources_use_effective_lane_profile_geometry",
    "parameters::actual::tests::sumeragi_v2_config_format_changes_the_handshake_fingerprint",
    "sumeragi::v2_core::refinement::tests::historical_body_pipeline_kernel_rejects_request_subject_and_owner_substitution",
    "sumeragi::v2_core::refinement::tests::historical_certificate_kernel_rejects_foreign_admission_and_unretired_request",
    "peer::run::tests::consensus_lane_and_v2_topics_share_authenticated_high_source_credit",
    "merge_sidecar::tests::exact_active_delivery_retry_preserves_decreasing_chunk_rank",
    "merge_sidecar::tests::alternate_source_progress_and_reconnect_preserve_independent_cursors",
    "merge_sidecar::tests::equal_ordinal_different_tenure_alternate_source_is_rejected_atomically",
    "merge_sidecar::tests::inactive_source_teardown_releases_budget_and_reconnect_resumes_cursor",
    "merge_sidecar::tests::later_delivery_preserves_the_current_source_cursor",
    "merge_sidecar::tests::later_delivery_while_chunk_is_in_flight_waits_for_flush_before_next_emit",
    "merge_sidecar::tests::late_old_exact_item_receipt_completes_reconnected_attempt_once",
    "merge_sidecar::tests::later_delivery_during_materialization_keeps_exact_authorized_route",
    "merge_sidecar::tests::writable_reconnect_during_materialization_keeps_exact_authorized_tenure",
    "state::tests::block_leaves_governance_unlock_audit_clean_when_no_locks_are_expired",
    "merge_sidecar::tests::equal_sequence_with_different_semantic_identity_is_rejected_before_materialization",
    "merge_sidecar::tests::transient_materialization_release_keeps_exact_retry",
    "merge_sidecar::tests::transient_response_capacity_defers_materialization_on_the_same_delivery",
    "merge_sidecar::tests::response_materialization_requires_and_consumes_its_exact_admission_gate",
    "merge_sidecar::tests::sidecar_admission_matches_the_cached_arc_without_changing_ownership",
    "merge_sidecar::tests::inactive_reply_route_is_rejected_before_server_gate_admission",
    "merge_sidecar::tests::completed_source_later_and_reconnect_stay_terminal_while_sibling_progresses",
    "merge_sidecar::tests::exact_delivery_retry_rematerializes_after_rate_gate_expiry",
    "merge_sidecar::tests::completed_source_does_not_block_a_new_alternate_source",
    "merge_sidecar::tests::configured_route_source_capacity_bounds_semantic_attempts",
    "merge_sidecar::tests::configured_source_geometry_reserves_more_than_eight_independent_attempts",
    "merge_sidecar::tests::third_session_from_one_hub_is_rejected_while_another_hub_progresses",
    "merge_sidecar::tests::source_byte_overflow_is_rejected_while_another_hub_progresses",
    "merge_sidecar::tests::completed_short_session_replacement_cannot_starve_an_older_long_session",
    "merge_sidecar::tests::route_retirement_between_admission_and_enqueue_releases_all_response_reservations",
    "merge_sidecar::tests::saturated_materializer_does_not_erase_same_request_alternate_session",
    "merge_sidecar::tests::saturated_materializer_does_not_erase_same_request_alternate_bytes",
    "merge_sidecar::tests::partitioned_materialization_preserves_rejected_source_resume_cursor",
    "merge_sidecar::tests::durable_response_drain_persists_pending_identity_before_handoff",
    "sumeragi::v2_lane_work::tests::durable_lane_certificate_is_one_atomic_kura_backed_response",
    "sumeragi::v2_lane_work::tests::durable_lane_certificate_serves_rotated_validator_after_pressure",
    "sumeragi::v2_lane_work::tests::historical_certificate_survives_successor_lock_decision_persistence_and_restart",
    "sumeragi::v2_lane_work::tests::carrier_replacement_filters_persistence_and_output_sources_together",
    "sumeragi::v2_lane_work::tests::applied_lane_certificate_retires_alternative_qc_replays_without_weakening_conflicts",
    "sumeragi::v2_lane_work::tests::native_amx_request_rejects_inactive_reply_route_before_signing",
    "sumeragi::v2_lane_work::tests::duplicate_reply_effect_preserves_exact_source_delivery",
    "sumeragi::v2_lane_work::tests::reply_effect_rejects_missing_or_retargeted_route_set",
    "sumeragi::v2_lane_work::tests::duplicate_reply_effect_updates_only_later_delivery_from_same_source",
    "sumeragi::v2_lane_work::tests::duplicate_reply_effect_retains_alternate_sources_across_source_update",
    "sumeragi::v2_lane_work::tests::temporarily_unserviceable_effect_requeues_behind_later_reserved_work",
    "sumeragi::v2_lane_work::tests::retired_sidecar_route_between_drain_and_lane_queue_preserves_live_sibling",
    "sumeragi::v2_runtime::tests::certified_tc_crosses_full_fence_blocked_prepare_prefix",
    "peer::run::tests::authenticated_source_credit_precedes_network_and_subscriber_backlogs",
    "peer::run::tests::recoverable_post_acknowledges_only_after_full_write_and_flush",
    "peer::run::tests::partial_write_error_closes_ack_without_false_completion",
    "peer::run::tests::coalesced_batch_acknowledges_every_item_only_after_flush",
    "peer::run::tests::maximum_frame_uses_a_bounded_number_of_source_reservations",
    "peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting",
    "peer::shared_byte_budget_tests::authenticated_source_count_registry_bounds_identity_churn_and_capacity_drift",
    "network::inbound_source_memory_bound_tests::authenticated_source_count_share_is_checked_and_never_zero",
    "network::tests::actor_progress_bypasses_full_deferred_owner_and_waits_for_writer_flush",
    "network::tests::actor_progress_lease_survives_topology_transition",
    "network::tests::actor_progress_retries_exactly_once_on_peer_writer_replacement",
    "network::tests::actor_progress_retry_round_robin_bypasses_partitioned_target",
    "network::tests::cap_one_blocked_source_cannot_prevent_live_source_service",
    "network::tests::actor_broadcast_retry_targets_only_failed_peers",
    "network::tests::reliable_subscriber_is_single_consumer_under_clone_budget_pressure",
    "network::tests::reconnecting_peer_cannot_multiply_retained_source_credits",
    "sumeragi::v2_core::refinement::tests::two_stage_relay_retry_kernel_rejects_source_rotation_eligibility_and_fifo_mutations",
    "tests::relay_fairness::daemon_source_credit_layers_over_upstream_and_preserves_the_ninth_exact_owner",
    "tests::relay_fairness::saturated_sumeragi_dispatch_does_not_hold_normal_worker_permits",
    "tests::relay_fairness::real_inner_ingress_retry_preserves_a_copies_and_bounds_b_service_rank",
    "sumeragi::status::v2_liveness_watchdog_tests::active_watchdog_is_deadline_driven_edge_triggered_and_recovers_on_progress",
    "sumeragi::status::v2_liveness_watchdog_tests::active_watchdog_resets_on_successor_owner_and_status_clear",
    "sumeragi::v2_runner::tests::synthesized_durable_rollover_contract_allows_successor_after_dead_target_handoff",
    "sumeragi::v2_runner::tests::reserved_lane_output_bypasses_unserviceable_head_without_losing_owner",
    "sumeragi::v2_runner::tests::runner_dispatch_preserves_durable_lane_certificate_reply_routes",
    "sumeragi::v2_runner::tests::runner_dispatch_preserves_certified_sidecar_chunk_reply_routes",
    "sumeragi::v2_runner::tests::bounded_sidecar_admission_turn_applies_only_its_budget",
    "sumeragi::v2_runner::tests::runner_dispatch_prunes_retired_sidecar_source_without_losing_live_sibling",
    "sumeragi::v2_runner::tests::runner_dispatch_advances_certified_sidecar_only_after_writer_flush",
    "sumeragi::v2_runner::tests::runner_dispatch_retired_admission_race_emits_no_sidecar_receipt",
    "sumeragi::v2_runner::tests::runner_closed_sidecar_flush_reconnect_retries_same_chunk_then_advances_once",
    "sumeragi::v2_runner::tests::closed_sidecar_prefix_handoff_requeues_only_failed_suffix",
    "sumeragi::v2_runner::tests::runner_dispatch_rejects_certified_sidecar_chunk_without_reply_route",
    "sumeragi::v2_runner::tests::runner_dispatch_rejects_durable_response_without_reply_routes",
    "sumeragi::v2_worker::tests::actor_backpressure_retains_exact_final_lane_commit_qc_post",
    "sumeragi::v2_worker::tests::actor_backpressure_retains_complete_merge_share_fanout",
    "sumeragi::v2_lifecycle_coordinator::tests::serve_retirement_activates_reserved_producer_before_later_admission",
    "sumeragi::v2_worker::tests::receiver_teardown_preserves_completion_pending_lifecycle_serve",
    "sumeragi::v2_worker::tests::receiver_teardown_rejects_queued_or_active_lifecycle_serve",
    "sumeragi::v2::tests::production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies",
    "sumeragi::v2_runner::lifecycle_height_driver::tests::completed_certified_serve_yields_before_the_next_outer_turn",
    "sumeragi::v2_worker::tests::same_tenure_updates_and_reconnect_preserve_current_item",
    "sumeragi::v2_worker::tests::closed_sidecar_source_reconnect_retries_current_item_while_sibling_backpressures",
    "sumeragi::v2_worker::tests::completed_sidecar_reconnect_preserves_terminal_cursor_without_capacity_charge",
    "sumeragi::v2_worker::tests::later_delivery_cannot_requeue_pending_or_unapplied_sidecar_flush_but_other_attempts_progress",
    "sumeragi::v2_worker::tests::mixed_source_retry_retains_terminal_flush_target_without_resetting_live_siblings",
    "sumeragi::v2_worker::tests::inactive_reply_target_tombstone_rejects_cross_source_equal_ordinal_collision",
    "sumeragi::v2_worker::tests::owned_reply_history_merge_retries_candidate_retirement_after_prune",
    "sumeragi::v2_worker::tests::newly_observed_alternate_hub_starts_at_zero_without_resetting_parked_source",
    "sumeragi::v2_worker::tests::a_b_a_hub_reconnect_preserves_each_source_cursor",
    "sumeragi::v2_worker::tests::owned_reply_transfer_retirement_after_validation_is_atomic",
    "sumeragi::v2_worker::tests::bulk_backpressure_does_not_block_reserved_lane_or_safety_output",
    "sumeragi::v2_worker::tests::non_roster_targets_cannot_consume_frozen_validator_reservations",
    "sumeragi::v2_worker::tests::partial_fanout_progress_releases_only_the_completed_target_unit",
    "sumeragi::v2_worker::tests::ownership_units_reject_reservation_spill_and_release_exact_target",
    "sumeragi::v2_worker::tests::backpressured_source_does_not_block_other_sources_or_consume_their_reserve",
    "sumeragi::v2_worker::tests::production_output_path_serves_later_fanout_while_target_stays_backpressured",
    "sumeragi::v2_worker::tests::response_outputs_without_exact_routes_fail_stop",
    "sumeragi::v2_worker::tests::sidecar_receipts_use_a_separate_bounded_control_queue",
    "sumeragi::v2_worker::tests::actor_backpressure_cannot_change_returned_payload_identity",
    "sumeragi::v2_worker::tests::exact_output_retry_rejects_a_different_message_identity",
    "sumeragi::v2_worker::tests::full_exact_output_corridor_does_not_disguise_non_progress_routes_as_backpressure",
    "sumeragi::v2_worker::tests::applied_height_handoff_retires_all_sidecar_flush_states_without_blocking_successor",
    "sumeragi::v2_worker::tests::applied_height_handoff_counts_and_clears_parked_reply_cursor_atomically",
    "sumeragi::v2_worker::tests::applied_height_handoff_rejects_output_without_reconstruction",
    "sumeragi::v2_worker::tests::applied_height_handoff_rejects_unbound_lane_output_atomically",
    "sumeragi::v2_worker::tests::applied_height_handoff_rejects_wrong_height_global_output",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_historical_kura_global_responses_atomically",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_only_exact_historical_kura_lane_certificate",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_kura_applied_ordinary_historical_lane_output",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_record_backed_autonomous_historical_lane_certificate",
    "peer::run::tests::dispatch_worker_shutdown_drains_reliable_old_generation_to_actor",
    "peer::run::tests::full_write_without_flush_ack_closes_actor_witness_and_retries_on_replacement",
    "network::handle_update_tests::progress_budget_preserves_fifo_for_three_registered_producers",
    "network::tests::reliable_progress_class_matches_actor_reservations_exactly",
    "network::tests::reply_route_survives_peer_message_clone_mapping_and_split",
    "network::tests::peer_message_rehydration_rejects_second_reply_route_without_retargeting",
    "network::tests::reply_source_key_groups_relay_origins_and_orders_actor_instances",
    "network::tests::reply_route_source_updates_are_ordinal_monotonic_and_target_scoped",
    "network::tests::dependent_test_fixture_mints_opaque_tenures_and_delivery_ordinals",
    "network::tests::cancelled_newer_hub_cannot_erase_older_independent_route_attempt",
    "network::tests::dependent_fixture_models_bounded_actor_global_multi_hub_ownership",
    "network::tests::reply_route_pruning_retains_equal_ordinal_tenure_tombstone",
    "network::tests::reply_route_binding_rejects_evicted_tombstone_collision",
    "network::tests::reply_route_set_isolates_sources_preserves_cursors_and_prunes_retired_capacity",
    "network::tests::route_cancelled_between_preflight_and_admission_retires_without_queue_ownership",
    "network::tests::reply_admission_rejects_retargeting_foreign_handles_and_wrong_tickets",
    "network::tests::reply_actor_admission_does_not_complete_writer_flush_ack",
    "network::tests::reply_flush_ack_cancellation_between_precheck_and_budget_lock_returns_none",
    "network::tests::retired_reply_tenure_closes_flush_ack_without_false_completion",
    "network::tests::reply_flush_test_fixture_distinguishes_success_timeout_and_close",
    "network::tests::reply_flush_ack_completes_only_after_peer_writer_flush",
    "network::handle_update_tests::progress_ticket_rejects_a_different_same_length_payload",
    "network::tests::configured_assist_hub_connection_cannot_overflow_reliable_geometry",
    "network::tests::topology_larger_than_reliable_target_geometry_is_rejected_atomically",
    "network::tests::assist_hub_refresh_above_reliable_geometry_is_rejected_atomically",
    "network::tests::topology_removal_cancels_every_deferred_owner_for_removed_peer",
    "network::tests::deferred_progress_survives_ttl_but_explicit_peer_removal_cancels_it",
    "network::tests::outside_topology_retransmit_is_not_misreported_as_delivered",
    "network::tests::accepted_draining_generation_delivers_reliable_progress_after_replacement",
    "network::handle_update_tests::targetized_broadcast_coalesces_only_the_same_digest_and_membership",
    "network::tests::distinct_broadcast_residual_is_target_isolated_and_its_rank_decreases",
    "network::tests::exact_broadcast_retry_coalesces_but_distinct_and_direct_requests_do_not",
    "network::tests::removed_membership_cancels_only_old_broadcast_debt_across_readd",
    "network::tests::cancelled_target_child_with_pending_flush_ack_releases_exactly_once",
    "network::tests::requested_topology_is_not_authority_and_closed_fanout_returns_all_targets",
    "network::tests::reliable_delivery_waits_for_its_route_subscriber",
    "network::tests::closed_reliable_subscriber_transfers_actor_pending_backlog_to_replacement",
    "network::tests::network_actor_drop_retires_routes_and_only_its_waiters",
    "consensus_message_control::tests::controlled_v2_admission_preserves_distinct_relay_identity",
    "network_relay_tests::test_control_hold_release_preserves_live_route_and_retires_canceled_reentry",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_exact_ownership_carrier_tracks_route_actions_and_cursors",
    "merge_sidecar::tests::sidecar_flush_refinement_advances_only_exact_source_chunk",
    "sumeragi::v2::tests::deferred_actor_source_never_aliases_across_adapter_instances",
    "sumeragi::v2::tests::deferred_adapter_rejects_foreign_and_replayed_capabilities_before_reducer_step",
    "sumeragi::v2::tests::deferred_authenticated_retry_retains_exact_original_and_effective_tags",
    "sumeragi::v2::tests::deferred_ordinal_exhaustion_fails_adapter_closed_before_wrap",
    "sumeragi::v2::tests::deferred_service_debt_overflow_is_typed_and_fail_closed",
    "sumeragi::v2::tests::deferred_service_evidence_rejects_every_owner_and_rank_mutation",
    "sumeragi::v2::tests::deferred_zero_ordinal_is_exact_single_use_and_never_reminted",
    "sumeragi::v2_effects::tests::live_runtime_step_rejects_missing_scheduler_ownership_before_callbacks",
    "sumeragi::v2_lane_work::tests::durable_lane_certificate_coalescing_preserves_alternate_ingress_owners",
    "sumeragi::v2_runtime::tests::adapter_command_identity_is_derived_from_exact_immutable_payload",
    "sumeragi::v2_runtime::tests::admission_ordinal_exhaustion_fails_runtime_closed",
    "sumeragi::v2_runtime::tests::runtime_rejects_replayed_foreign_and_mutated_deferred_tokens",
    "sumeragi::v2_runtime::tests::scheduler_owner_carrier_covers_live_and_typed_deferred_branches",
    "sumeragi::v2_runtime::tests::scheduler_owner_carrier_pins_exact_fifo_identity_and_rank_fields",
    "sumeragi::v2_runtime::tests::scheduler_owner_must_be_taken_before_a_later_step_can_enter",
    "sumeragi::v2_runtime::tests::selected_owner_without_a_runtime_minted_ordinal_fails_closed",
    "sumeragi::v2_worker::tests::exact_output_coalescing_preserves_distinct_fair_ingress_admissions",
    "sumeragi::v2_worker::tests::orphan_chunk_coalescing_preserves_alternate_fair_ingress_routes",
    "sumeragi::v2_worker::tests::sidecar_flush_ack_identity_mismatch_fails_closed",
    "network::tests::reply_flush_identity_binds_ticket_tenure_source_payload_and_delivery_occurrence",
    "network::tests::reply_flush_test_fixture_binds_exact_canonical_post_and_opaque_actor",
    "consensus_message_control::tests::failed_release_clears_in_flight_ownership_and_latches_fatal",
    "consensus_message_control::tests::fatal_controller_rejects_an_unchanged_command_poll",
    "consensus_message_control::tests::retired_release_finishes_drain_without_claiming_delivery",
    "network_relay_tests::obsolete_block_ingress_disposition_fails_closed",
    "network_relay_tests::obsolete_lane_ingress_disposition_fails_closed",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_projection_distinguishes_identical_bytes_from_distinct_origins",
    "merge_sidecar::tests::authenticated_source_limits_are_fixed_at_four_gates_two_sessions_and_sixteen_mibibytes",
    "sumeragi::v2::tests::authenticated_deferred_service_rejects_same_kind_envelope_swap_before_reducer",
    "sumeragi::v2_effects::tests::owned_payload_chunk_rejects_source_swap_before_service_and_keeps_unknown_work_nonfatal",
    "sumeragi::v2_runtime::tests::runtime_merges_alternate_sources_for_one_semantic_request",
    "sumeragi::v2_runtime::tests::runtime_keeps_identical_wire_requests_from_distinct_semantic_origins_independent",
    "sumeragi::v2_runtime::tests::busy_deferred_request_merges_alternate_source_and_services_exact_carrier",
    "sumeragi::v2_worker::tests::owned_orphan_chunk_replay_preserves_alternate_source_routes_and_cursors",
    "network::tests::peer_message_mints_actor_global_delivery_ordinals_across_connection_tenures",
    "parameters::actual::tests::sumeragi_v2_exact_output_geometry_checks_every_arithmetic_boundary",
    "parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_accepts_network_source_boundary",
    "parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_accepts_equal_capacity_boundary",
    "parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_rejects_unreservable_network_sources",
    "sumeragi::v2_core::tests::future_view_commit_qc_uses_current_owner_through_application",
    "sumeragi::v2_core::tests::later_view_commit_qc_replays_and_applies_the_retained_lock_origin",
    "sumeragi::v2_core::tests::height_context_rejects_invalid_parent_proposal_origin_geometry",
    "sumeragi::v2_core::tests::stale_generation_completion_is_rejected_after_view_change",
    "sumeragi::v2_core::tests::stale_persistence_completions_stutter_while_current_append_is_pending",
    "sumeragi::v2_core::tests::strictly_ahead_install_timeout_advances_owner_and_protects_highest_prepare",
    "sumeragi::v2_core::tests::same_round_timeout_with_strictly_higher_prepare_rebinds_lock_without_view_change",
    "sumeragi::v2_core::tests::later_lock_and_commit_ack_retires_older_same_origin_commit_pool",
    "sumeragi::v2_core::tests::validated_tc_lock_survives_current_view_timeout_and_commits_after_next_tc",
    "sumeragi::v2_core::tests::replay_resigns_the_newest_commit_intent_for_one_proposal_origin",
    "sumeragi::v2_core::refinement::tests::durable_intent_refinement_accepts_exact_stutters_and_rejects_mutations",
    "sumeragi::v2_core::refinement::tests::locked_commit_progress_witness_accepts_exact_owners_and_rejects_mutations",
    "sumeragi::v2_core::reducer::source_link_tests::certified_fetch_capability_requires_the_exact_proposal_origin",
    "sumeragi::v2_core::reducer::tests::historical_commit_cannot_cross_the_current_finality_timeout_fence",
    "sumeragi::v2_core::wal::byte_lifecycle_tests::same_round_timeout_replay_accepts_only_a_strict_prepare_origin_upgrade",
    "sumeragi::evidence::tests::sumeragi_v2_equivocation_authenticates_vote_origin_and_execution",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_ownership_projection_ignores_route_liveness_until_maintenance",
    "sumeragi::v2::tests::deferred_projection_distinguishes_authenticated_proposal_origins",
    "sumeragi::v2::tests::vote_body_ownership_uses_the_authenticated_proposal_origin",
    "sumeragi::v2::tests::locked_subject_is_safe_only_at_its_exact_proposal_origin",
    "sumeragi::v2_effects::tests::later_commit_qc_applies_the_exact_retained_lock_origin",
    "sumeragi::v2_effects::tests::later_view_commit_signing_uses_the_fsynced_proposal_origin_marker",
    "sumeragi::v2_lane_work::tests::prior_height_hydration_stays_local_under_successor_backpressure",
    "sumeragi::v2_runner::tests::first_same_subject_lock_from_prior_view_retires_unlocked_work",
    "sumeragi::v2_runtime::tests::exact_authenticated_qc_from_distinct_sources_coalesces_in_one_runtime_slot",
    "sumeragi::v2_runtime::tests::same_semantic_qc_with_conflicting_route_authority_fails_closed_atomically",
    "sumeragi::v2_runtime::tests::runtime_ingress_carrier_capacity_returns_backpressure_atomically",
    "sumeragi::v2_transport::tests::later_commit_qc_authenticates_the_exact_locked_body_origin",
    "zk::kagemusha_finality::tests::aggregate_signature_authenticates_proposal_origin",
    "block::consensus_v2::finality::tests::header_binding_requires_exact_origin_but_allows_later_certification",
    "block::consensus_v2::finality::tests::genesis_header_binding_accepts_a_later_first_proposal_origin",
    "offline::kagemusha_v4_topup_provenance_tests::compact_qc_rejects_foreign_or_future_proposal_origin",
    "block::consensus_v2::tests::height_context_identity_authenticates_the_parent_proposal_origin",
    "sumeragi_v2_runner::prepare_qc_split_tests::restart_scenario_uses_a_contention_tolerant_view_zero_deadline",
    "sumeragi::v2::tests::successor_core_context_preserves_the_parent_certificate_binding",
    "sumeragi::v2_lane_work::tests::decided_lane_ownership_blocks_rollover_until_its_session_is_durable",
    "sumeragi::v2_recovery::tests::finality_complete_tip_with_incomplete_lane_completion_reopens_same_height",
    "sumeragi::v2_runner::tests::terminal_ingress_discards_commit_discovery_and_losing_current_body_requests",
)
_PRODUCTION_LIVENESS_RETIRED_REGRESSIONS = frozenset(
    (
        "merge_sidecar::tests::equal_ordinal_different_tenure_alternate_source_is_rejected_atomically",
        "merge_sidecar::tests::inactive_source_teardown_releases_budget_and_reconnect_resumes_cursor",
        "merge_sidecar::tests::partitioned_materialization_preserves_rejected_source_resume_cursor",
        "merge_sidecar::tests::exact_delivery_retry_rematerializes_after_rate_gate_expiry",
        "sumeragi::v2_worker::tests::mixed_source_retry_retains_terminal_flush_target_without_resetting_live_siblings",
        "peer::run::tests::dispatch_worker_shutdown_drains_reliable_old_generation_to_actor",
        "network::tests::accepted_draining_generation_delivers_reliable_progress_after_replacement",
        "sumeragi::v2_core::tests::later_view_commit_qc_replays_and_applies_the_retained_lock_origin",
        "sumeragi::v2_core::tests::height_context_rejects_invalid_parent_proposal_origin_geometry",
        "sumeragi::v2_core::tests::same_round_timeout_with_strictly_higher_prepare_rebinds_lock_without_view_change",
        "sumeragi::v2_core::tests::later_lock_and_commit_ack_retires_older_same_origin_commit_pool",
        "sumeragi::v2_core::tests::validated_tc_lock_survives_current_view_timeout_and_commits_after_next_tc",
        "sumeragi::v2_core::tests::replay_resigns_the_newest_commit_intent_for_one_proposal_origin",
        "sumeragi::v2_core::reducer::tests::historical_commit_cannot_cross_the_current_finality_timeout_fence",
        "sumeragi::v2::tests::locked_subject_is_safe_only_at_its_exact_proposal_origin",
        "sumeragi::v2_effects::tests::later_commit_qc_applies_the_exact_retained_lock_origin",
        "sumeragi::v2_effects::tests::later_view_commit_signing_uses_the_fsynced_proposal_origin_marker",
        "sumeragi::v2_transport::tests::later_commit_qc_authenticates_the_exact_locked_body_origin",
        "block::consensus_v2::finality::tests::header_binding_requires_exact_origin_but_allows_later_certification",
        "block::consensus_v2::finality::tests::genesis_header_binding_accepts_a_later_first_proposal_origin",
        "block::consensus_v2::tests::height_context_identity_authenticates_the_parent_proposal_origin",
        "sumeragi_v2_runner::prepare_qc_split_tests::restart_scenario_uses_a_contention_tolerant_view_zero_deadline",
        "sumeragi::v2::tests::successor_core_context_preserves_the_parent_certificate_binding",
        "sumeragi::v2_lane_work::tests::decided_lane_ownership_blocks_rollover_until_its_session_is_durable",
        "sumeragi::v2_recovery::tests::finality_complete_tip_with_incomplete_lane_completion_reopens_same_height",
        "sumeragi::v2_runner::tests::terminal_ingress_discards_commit_discovery_and_losing_current_body_requests",
    )
)
_PRODUCTION_LIVENESS_POSTCUT_REGRESSIONS = (
    "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::locked_publication_fence_serializes_same_wire_and_reenqueues_after_commit",
    "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::locked_publication_fence_serializes_unrelated_append_and_preserves_it",
    "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::dropping_locked_publication_fence_releases_producer_without_dequeue",
    "merge_sidecar::tests::reused_actor_ordinals_under_different_tenures_are_rejected_atomically",
    "merge_sidecar::tests::reply_unwritable_route_parks_inflight_materialization_without_bytes",
    "merge_sidecar::tests::exact_delivery_retry_stays_terminal_beyond_retired_ttl_horizon",
    "merge_sidecar::tests::unsent_request_restores_holder_and_backoff_state",
    "merge_sidecar::tests::idle_request_retry_starts_strictly_after_the_fairness_cursor",
    "merge_sidecar::tests::request_stream_close_floor_advances_only_over_a_contiguous_terminal_prefix",
    "merge_sidecar::tests::authenticated_close_floor_retires_covered_output_and_rejects_replay_or_regression",
    "merge_sidecar::tests::rejected_request_does_not_consume_server_stream_state",
    "merge_sidecar::tests::height_rollover_retries_only_each_sources_current_in_flight_chunk",
    "merge_sidecar::tests::durable_requester_restart_advances_sequence_and_carries_close_floor",
    "merge_sidecar::tests::durable_requester_crash_before_send_closes_unobserved_sequence",
    "merge_sidecar::tests::durable_stream_epochs_and_service_generations_bound_peer_churn",
    "merge_sidecar::tests::durable_lifecycle_rejects_canonical_payload_with_stale_digest",
    "merge_sidecar::tests::durable_responder_restart_preserves_same_hub_gate_budget",
    "merge_sidecar::tests::durable_responder_restart_allows_new_source_while_recovered_source_is_offline",
    "merge_sidecar::tests::durable_responder_restart_preserves_terminal_source_cursor_and_rebinds_capability",
    "sumeragi::v2_lane_work::tests::sidecar_lifecycle_journal_failure_latches_restart_before_request_dispatch",
    "sumeragi::v2_lane_work::tests::sidecar_close_journal_failure_latches_restart_and_blocks_queued_chunk",
    "sumeragi::v2_lane_work::tests::sidecar_close_ack_journal_failure_latches_restart_before_completion",
    "sumeragi::v2_lane_work::tests::sidecar_timeout_journal_failure_latches_restart_before_retry_dispatch",
    "sumeragi::v2_worker::tests::delayed_old_tenure_delivery_cannot_replace_newer_worker_reply_route",
    "sumeragi::v2_worker::tests::ordinary_reply_timeout_grows_only_its_source_attempt_while_sibling_progresses",
    "sumeragi::v2_worker::tests::ordinary_reply_late_old_flush_after_reconnect_advances_exactly_once",
    "sumeragi::v2_worker::tests::mixed_source_retry_retains_pending_flush_target_without_resetting_live_siblings",
    "peer::run::tests::dispatch_worker_shutdown_drains_reliable_replaced_connection_to_actor",
    "network::tests::delayed_superseded_tenure_cannot_replace_or_tombstone_newer_same_source_writer",
    "network::tests::reply_wrapper_exposes_delivery_active_unwritable_no_ownership",
    "network::tests::accepted_draining_connection_delivers_reliable_progress_after_replacement",
    "network::tests::reply_route_tenure_retires_only_after_final_receiver_guard_drops",
    "consensus_message_control::tests::private_reader_treats_safe_atomic_replacement_as_retryable_identity_churn",
    "network_relay_tests::certified_merge_sidecar_close_is_limited_but_responder_controls_are_critical",
    "sumeragi::v2::tests::strict_same_round_tc_preserves_and_retags_timeout_vote_owners",
    "sumeragi::v2_core::tests::later_reproposal_commit_qc_replays_and_applies_its_exact_certified_round",
    "sumeragi::v2_core::tests::valid_commit_qc_supersedes_different_subject_prepare_lock_live_and_replay",
    "sumeragi::v2_effects::tests::different_subject_decision_supersedes_protected_lock_and_frees_losing_capacity",
    "sumeragi::v2_effects::tests::apply_rejects_matching_commit_qc_from_foreign_context_without_scheduling_work",
    "sumeragi::v2_core::tests::height_context_requires_one_same_round_parent_commit_geometry",
    "sumeragi::v2_core::tests::same_round_timeout_upgrade_rebinds_lock_and_retains_current_timeout_vote",
    "sumeragi::v2_core::tests::later_reproposal_commit_ack_retires_durable_old_round_commit_pool",
    "sumeragi::v2_core::tests::tc_lock_survives_closed_view_and_commits_after_later_same_subject_reproposal",
    "sumeragi::v2_core::tests::replay_resigns_same_subject_reproposal_fifo_without_relabelling_old_commit",
    "sumeragi::v2_core::refinement::tests::strict_same_round_refinement_kernels_reject_split_round_mutations",
    "sumeragi::v2_core::refinement::tests::wal_retirement_authorization_rejects_split_round_decision_and_receipt",
    "sumeragi::v2_core::refinement::tests::semantic_commit_decision_identity_ignores_only_qc_rounds",
    "sumeragi::v2_core::reducer::source_link_tests::closed_proposal_round_cannot_create_a_new_commit_intent",
    "sumeragi::v2::tests::locked_subject_reproposal_and_strict_higher_prepare_are_safe",
    "sumeragi::v2::tests::successor_context_requires_the_durable_cryptographic_parent",
    "sumeragi::v2::tests::authentication_rejects_valid_commitment_conflicts_without_mutating_adapter",
    "sumeragi::v2_effects::tests::authenticated_genesis_satisfies_manifestless_certified_decision_fetch_locally",
    "sumeragi::v2_effects::tests::recovered_next_vote_body_catalog_join_is_exact_and_store_bound",
    "sumeragi::v2_effects::tests::reproposal_commit_signing_uses_its_same_round_validation_marker",
    "sumeragi::v2_runtime::tests::exact_authenticated_timeout_certificate_coalesces_then_applies_through_signer",
    "sumeragi::v2_runtime::tests::body_available_rebind_accepts_same_view_higher_generation",
    "sumeragi::v2_transport::tests::reproposal_commit_qc_authenticates_its_exact_same_round_body",
    "block::consensus_v2::finality::tests::header_binding_allows_unchanged_reproposal_but_rejects_earlier_decision_round",
    "block::consensus_v2::tests::height_context_identity_ignores_reproposal_round_and_rejects_split_rounds",
    "sumeragi::v2_core::tests::vote_statement_identity_excludes_only_the_authenticated_signer",
    "sumeragi::v2_core::tests::certificate_height_subject_identity_ignores_round_and_phase_only",
    "sumeragi::v2_core::tests::view_zero_binds_semantic_parent_decision_across_reproposal_rounds",
    "sumeragi::v2_core::tests::earlier_same_body_commit_qc_supersedes_a_later_reproposal_lock",
    "sumeragi::v2::tests::registry_rejects_split_round_vote_and_qc_reference",
    "sumeragi::v2_body_store::tests::rotating_leader_locked_body_reproposal_is_stored_and_revalidated_per_round",
    "sumeragi::v2_body_store::tests::rotating_leader_reproposal_authenticates_the_immutable_header_leader",
    "sumeragi::v2_effects::tests::recovered_validation_catalog_hydrates_direct_apply_durability",
    "sumeragi::v2_effects::tests::split_round_commit_signing_is_rejected_before_service_dispatch",
    "sumeragi::v2_runner::tests::exact_locked_body_is_reencoded_at_the_reproposal_round_without_byte_drift",
    "sumeragi::v2_runner::tests::recovered_lifecycle_proposal_attempt_suppresses_same_view_after_lock_upgrade",
    "sumeragi::v2_worker::tests::production_exact_output_observes_finality_only_after_state_commit",
    "sumeragi::v2_worker::tests::applied_height_finality_releases_only_ticketless_global_topology_target",
    "sumeragi::v2_worker::tests::closed_flush_on_delivery_active_unwritable_route_parks_without_cursor_advance",
    "sumeragi::v2_worker::tests::closed_flush_racing_final_receiver_retirement_is_nonfatal",
    "sumeragi::v2_worker::tests::unavailable_admission_racing_retirement_is_nonfatal",
    "sumeragi::v2_worker::tests::entered_view_accepts_same_view_higher_generation_supersession",
    "peer::run::tests::peer_task_abort_drains_queued_worker_then_notifies_exact_connection_once",
    "peer::run::tests::peer_task_panic_closes_delivery_producer_and_notifies_exact_connection_once",
    "peer::run::tests::dispatch_worker_join_error_is_returned_after_fail_closed_teardown",
    "network::tests::duplicate_configured_termination_does_not_advance_backoff_or_metrics",
    "sumeragi::v2_lane_work::tests::late_old_sidecar_flush_removes_only_reconnected_source_retry",
    "tests::relay_fairness::hold_release_same_source_reconnect_retires_old_delivery_without_rebinding_new_route",
    "network::tests::deferred_queue_preserves_order_and_connection_bindings",
    "network::tests::flush_deferred_frames_closed_session_restores_remaining_unbound",
    "network::tests::flush_deferred_frames_drops_stale_connection_binding_without_posting",
    "network::tests::flush_deferred_frames_rebinds_reliable_stale_connection",
    "network::tests::flush_deferred_frames_sends_unbound_entries_to_current_session",
    "network::tests::live_session_backpressure_defers_retry_with_current_connection",
    "network::tests::live_session_closed_defers_retry_unbound_and_removes_peer",
    "network::tests::live_session_post_overflow_disconnect_policy_defers_unbound",
    "network::tests::missing_session_retains_unbound_consensus_frame_and_schedules_reconnect",
    "network::tests::rejected_authenticated_connection_is_cancelled_and_remains_cap_accounted",
    "sumeragi::v2_core::tests::recovery_excludes_proposal_intent_superseded_by_same_round_timeout_upgrade",
    "sumeragi::v2_core::tests::recovery_uses_same_round_timeout_upgrade_as_exact_local_proposal_justification",
    "sumeragi::v2_core::tests::replay_accepts_strictly_higher_matching_prepare_qc_proposal",
    "sumeragi::v2_core::tests::replay_resigns_proposal_with_equivalent_parent_reproposal_round",
    "sumeragi::v2_core::tests::same_round_timeout_upgrade_is_exact_local_proposal_justification",
    "sumeragi::v2_core::refinement::tests::decision_ack_retires_competing_owners_and_keeps_one_body_pipeline",
    "sumeragi::v2_core::refinement::tests::lock_and_commit_requires_one_current_vote_and_proposal_round",
    "block::consensus_v2::tests::timeout_proposal_accepts_only_the_selected_prepare_subject",
    "sumeragi::v2_core::tests::future_prepare_qc_is_transactionally_ignored_without_retransmit_ownership",
    "sumeragi::v2_core::tests::tc_omitting_the_local_high_keeps_its_exact_prepare_qc_retransmittable",
    "sumeragi::v2_core::reducer::source_link_tests::enter_view_projection_selects_and_fetches_the_exact_post_install_lock",
    "sumeragi::v2_core::reducer::source_link_tests::enter_view_without_a_lock_carries_and_fetches_nothing",
    "sumeragi::v2_core::reducer::source_link_tests::enter_view_effect_cannot_substitute_an_equal_reference_certificate",
    "merge_sidecar::tests::full_server_table_never_advances_generation_without_a_changed_roster",
    "sumeragi::v2_lane_work::tests::duplicate_generation_hint_coalesces_alternate_reply_sources",
    "sumeragi::v2_runner::tests::relayed_generation_hint_preserves_reply_route_from_lane_through_worker",
    "sumeragi::v2_worker::tests::generation_hint_requires_exact_reply_route_ownership",
    "network_relay_tests::certified_merge_sidecar_messages_preserve_ingress_reply_route",
    "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::turn_cut_dequeues_exact_winner_once_and_preserves_ready_rotation",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_request_cutoff_blocks_later_same_source_serve",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_request_cutoff_blocks_later_churn",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_occurrence_ordinal_coalesces_and_overflow_closes",
    "sumeragi::authoritative_runtime_gate_tests::restored_productive_retry_stays_behind_an_earlier_certified_request_carrier",
    "sumeragi::v2_certified_serve_payload_store::tests::authenticated_cut_rejects_a_later_valid_payload_from_a_second_store_owner",
    "sumeragi::v2_certified_serve_payload_store::tests::capacity_is_checked_before_a_second_file_is_published",
    "sumeragi::v2_certified_serve_payload_store::tests::completed_payload_requires_exact_certified_responder_authority",
    "sumeragi::v2_certified_serve_payload_store::tests::authenticated_cut_rejects_store_directory_symlink_replacement",
    "sumeragi::v2_certified_serve_payload_store::tests::completed_payload_requires_exact_durable_body_receipt_and_bytes",
    "sumeragi::v2_certified_serve_payload_store::tests::negative_terminal_is_idempotent_and_cannot_be_replaced",
    "sumeragi::v2_certified_serve_payload_store::tests::only_the_call_that_created_pending_owns_preledger_abort_authority",
    "sumeragi::v2_certified_serve_payload_store::tests::pending_receipt_requires_verified_qc_and_local_retention_authority",
    "sumeragi::v2_certified_serve_payload_store::tests::recovery_cut_reauthenticates_request_qc_and_typed_negative",
    "sumeragi::v2_certified_serve_payload_store::tests::recovery_cut_reconstructs_and_authenticates_completed_response",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::frame_roundtrip_is_canonical_and_preserves_high_water",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::one_signed_serve_request_cannot_own_two_lifecycle_pairs",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::orphan_serve_or_producer_records_are_rejected",
    "sumeragi::v2_lifecycle_coordinator::projection::tests::cancelled_certified_serve_tombstone_replays_with_its_terminal_producer_pair",
    "sumeragi::v2_lifecycle_coordinator::projection::tests::certified_serve_completion_settles_from_the_post_fsync_response_receipt",
    "sumeragi::v2_lifecycle_coordinator::projection::tests::certified_serve_negative_settlement_requires_the_exact_post_fsync_receipt",
    "sumeragi::v2_lifecycle_coordinator::projection::tests::certified_serve_rejects_a_receipt_for_another_signed_request",
    "sumeragi::v2_lifecycle_coordinator::projection::tests::certified_serve_terminal_family_mismatch_fails_without_state_mutation",
    "sumeragi::v2_lifecycle_coordinator::projection::tests::pending_certified_serve_admits_one_ready_serve_and_adjacent_dormant_producer",
    "sumeragi::v2_lifecycle_coordinator::replay_authority::tests::certified_serve_pending_replay_pair_binds_exact_fsync_origin_and_records",
    "sumeragi::v2_lifecycle_coordinator::replay_authority::tests::recovered_serve_states_reconstruct_one_common_source_per_replay_pair",
    "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::post_cut_append_preserves_geometry_but_pre_cut_mutation_fails_cas",
    "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::prepared_commit_preserves_unrelated_post_cut_append",
    "sumeragi::v2_lifecycle_coordinator::launch::turn_driver::ordinary_ingress_token_tests::armed_token_closes_output_before_releasing_dequeued_carrier_and_serve_result",
    "sumeragi::v2_lifecycle_coordinator::open::recovery_tests::complete_tip_serve_reconciliation_binds_the_exact_source_frame",
    "sumeragi::v2_lifecycle_coordinator::open::recovery_tests::complete_tip_serve_reconciliation_rejects_missing_final_cut_coverage",
    "sumeragi::v2_lifecycle_coordinator::scheduler_inputs::certified_serve_scheduler_tests::certified_serve_claim_rolls_back_when_its_exact_carrier_drifted",
    "sumeragi::v2_lifecycle_coordinator::scheduler_inputs::certified_serve_scheduler_tests::certified_serve_scheduler_creates_exactly_one_live_claim",
    "sumeragi::v2_lifecycle_coordinator::tests::capacity_fence_freezes_the_complete_serve_companion",
    "sumeragi::v2_lifecycle_coordinator::tests::durable_rollover_rejects_live_serve_without_payload_cancellation_receipt",
    "sumeragi::v2_lifecycle_coordinator::tests::recovery_requires_a_bijective_atomic_serve_producer_pair",
    "sumeragi::v2_lifecycle_coordinator::tests::restart_derives_ready_producer_debt_from_terminal_serve",
    "sumeragi::v2_lifecycle_coordinator::tests::serve_and_producer_share_one_reconstruction_source",
    "sumeragi::v2_lifecycle_coordinator::tests::serve_and_producer_terminalization_fail_closed_without_the_atomic_debt",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_checked_dequeue_freezes_one_physical_cut_per_occurrence",
    "sumeragi::v2::tests::deferred_occurrence_capability_binds_direct_authenticated_provenance",
    "sumeragi::v2_runtime::tests::runtime_rejects_driver_selection_outside_eligible_deferred_owner_set",
    "sumeragi::v2_runtime::tests::runtime_physical_cut_is_monotone_and_regression_fails_closed",
    "sumeragi::v2_runtime::tests::deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences",
    "sumeragi::v2_runtime::tests::post_cut_old_logical_replay_cannot_overtake_fenced_busy_deferred_target",
    "sumeragi::v2_runtime::tests::pre_dequeue_probe_validates_unfrozen_leader_wire_identity",
    "sumeragi::v2_runtime::tests::busy_deferred_older_aggregate_rebases_owner_and_rejects_identity_mutation",
    "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::fresh_certified_serve_rejects_foreign_target_and_rolls_back_capacity_wait",
    "sumeragi::v2_effects::tests::fetch_retransmissions_reuse_one_work_slot_and_one_signed_request",
    "sumeragi::v2_effects::tests::apply_retransmissions_reuse_one_work_slot",
    "sumeragi::v2_runtime::tests::distinct_pre_runtime_leader_wire_qc_waits_behind_busy_deferred_owner",
    "sumeragi::v2_worker::tests::durable_reconstructed_body_terminalizes_late_chunk_across_arrival_order",
    "sumeragi::v2_worker::tests::productive_retry_after_proofless_reconstruction_does_not_become_orphan",
    "sumeragi::v2_lifecycle_coordinator::launch::tests::recovered_decision_fetch_composite_dispatch_reserves_capacity_before_claim_and_commit",
    "sumeragi::v2_lane_work::tests::native_amx_manifest_projects_finality_bound_merge_batch_in_canonical_order",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_excludes_coordinator_only_receipts",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_same_route_identity_conflict",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_duplicate_group_source",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_matches_decoded_replay_entry",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_multiple_participant_heights_in_one_carrier",
    "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_same_height_participant_identity_conflict",
    "kura::tests::exact_retired_autonomous_attempt_accessor_uses_proposal_height_namespace",
    "sumeragi::v2_lane_work::tests::decided_mixed_carrier_accepts_canonical_successor_while_local_sidecars_lag",
    "sumeragi::v2_lane_work::tests::cold_restart_hydrates_two_link_raw_lane_chain_without_receipts",
    "sumeragi::v2_worker::tests::applied_height_handoff_retires_only_exact_same_finality_nonwinning_autonomous_outputs_atomically",
    "sumeragi::v2::tests::ready_local_proposal_sign_and_exact_output_precede_pending_timeout_certificate",
    "sumeragi::v2_runner::lifecycle_height_driver::tests::only_an_eligible_claim_can_preempt_an_ordinary_head_for_ready_proposal_sign",
    "queue::tests::replica_disposition_observes_exact_fifo_beneath_global_selection_overlay",
)
_PRODUCTION_LIVENESS_NEW_REGRESSIONS = tuple(
    test_name
    for test_name in _PRODUCTION_LIVENESS_NEW_REGRESSIONS
    if test_name not in _PRODUCTION_LIVENESS_RETIRED_REGRESSIONS
) + _PRODUCTION_LIVENESS_POSTCUT_REGRESSIONS

# The retained executor queue is the concrete FIFO consumer for reducer
# batches. Bind the complete reviewed methods, then additionally check the
# semantic ordering fragments below so failures identify the violated seam.
_PRODUCTION_RETAINED_EFFECT_FIFO_ITEM_SHA256 = {
    "consume_effects_with_runner_decision_cleanup": (
        "d92a98392155a733a9e8e00b3a5cb1de5fbc57c886cf96ad399a4781690bbc5a"
    ),
    "new_decision_batch_has_only_exact_apply": (
        "95f2f1c852290fd5037f6b88797e6c15982672bffb1a635a18a683e285e2caf1"
    ),
    "plan_runner_decision_cleanup": (
        "03094a082a2ba665c7cc655dd4fdcd68e98d4ec0499f7e127d8f16ad7d4455c5"
    ),
    "acknowledge_runner_decision_cleanup": (
        "d0a366f336e417fd1fcaecd51cfc8500f11878417f969ee97c6e2affbac545d2"
    ),
    "retain_effect_batch": (
        "3976c357aa3b66c71eac8bf8003bab79e024f76e9e469468bb8fb86c4c58dcfe"
    ),
    "retain_effect_batch_at_frontier": (
        "59ff1532bc4f12491fc9aaaa433c47bdb36b4560399548a4862362ea10576d00"
    ),
    "preflight_effect_batch_frontier": (
        "9001146fbf12cc9a6e6a1d6adbc68ca59d697edfeea9124b9d5a09d82c8d0624"
    ),
    "prepare_parked_effects_for_frontier": (
        "0c71370501ce7e04b6ab7abbad10ef79b471d74c9f20f8ada3e512d4514671a9"
    ),
    "commit_reconciliation_frontier": (
        "c6277d6ab0e71c9e7149921c6dc6fe6e3fe53e3eb317e624e3e6b490d9543bb7"
    ),
    "drain_retained_effect_batch": (
        "bf4bc14d8826b67a427dc1881a5398740ce6fcbb7b7e11ddccf7adc0b0165f94"
    ),
    "consume_pacemaker_effects_with_runner_decision_cleanup": (
        "b89bf586aec000a7a5b01cd8dbddb774ab7dc07f74adc64df7b07cb379ae03b9"
    ),
    "step_pacemaker_once": (
        "c35c6e284b25bc13c1e08d351b5108be7ba72e51701b8beab9860765337bc163"
    ),
    "step": "4e20148058bb31e701479b340112033023b2e2b8da1ec2677734d936f4053a15",
    "step_pending_tip_recovery": (
        "839907e02db4e0fdda25a94c9d17b3b98b8884e0037eb5038d45899d98c4c05e"
    ),
}

_RETAINED_EFFECT_PROGRESS_INGRESS_REGRESSION_SHA256 = (
    "316bd1d4d3d9771f3566abaf6e77f07ad80d1d74fb1f5186875a4a3d30d1b88c"
)

# Lifecycle Decision Apply uses a durable lifecycle corridor alongside the
# runner Decision-cleanup fence. Bind the complete corridor: availability must
# freeze all executor debt, dispatch must bind the exact recovered lineage,
# completion preparation must authenticate lineage, and finality may publish
# only after those owners have drained.
_PRODUCTION_LIFECYCLE_DECISION_APPLY_ITEM_SHA256 = {
    "lifecycle_decision_apply_dispatch_available": (
        "7e2cda73ed33428f470058749765f60ddbc1498fe3e5139dd20e274063127faf"
    ),
    "prepare_lifecycle_decision_apply_executor_dispatch": (
        "aa50a92c3308f82cf1e31a5fb2746668c9584b9267c00aa21d5a406ca96a3de5"
    ),
    "prepare_lifecycle_decision_apply_completion": (
        "49d307803293b19e905bd8b7153ee043dc2835288619293d12f194642791a2d7"
    ),
    "commit_lifecycle_decision_apply_finality": (
        "9689f8d90c03e75fe9397df07d650b9c7a9fd0b98c120c82acdda16c0f4999c0"
    ),
}

_PRODUCTION_EFFECT_SCHEDULER_HANDOFF_ITEM_SHA256 = {
    "take_scheduler_ownership": (
        "bccdd7c65959f3921d46373c86d1d6545568a326ddea42379362c7b1970397cf"
    ),
}

# Exact comment/literal-free token digests for the production exact-output
# scheduler and applied-height retirement seam.  The scheduler digests bind
# target-local/global FIFO ownership, round-robin service, and exact returned
# post retention.  The retirement digests bind the narrow Retransmit whitelist
# and the Kura receipt/finality-artifact authority checked before output is
# handed to durable reconstruction. Autonomous retirement additionally binds
# the current-height noncanonical fallback to one immutable retired attempt and
# the same finalized carrier's nonwinning authority.
_PRODUCTION_EXACT_OUTPUT_ITEM_SHA256 = {
    "autonomous_new_view_body_matches_durable_payload": (
        "d1720512781a97bde960a6467e59016e15310cf69047787a5176350386e371e8"
    ),
    "autonomous_lane_output_matches_payload_identity": (
        "6ad31433c9592ac390c299fb4bc0d7174b9dc36cde05a8168f208482a6ac9d18"
    ),
    "autonomous_lane_output_has_exact_retirement_source": (
        "f160be0e0bcce5a09813d9b1ce971c361991bff4ac9a33469a5130bbd11b80f6"
    ),
    "autonomous_lane_output_has_durable_reconstruction_source": (
        "0d806079c721d886405ad801b9fcb145b81387c2b9bc757534c82a4295a60e82"
    ),
    "take_attempt": "acc18d3997a0cc6fcca4926b72a63fedf5d0987ecb33c1114e93e0da3b2254d7",
    "mark_admitted": "c6e502433ef5249540446d75e0f88f665a7ffc456bf4014216f808fd123f072c",
    "retain_returned": "8f1436db10edaa22360b416024817e8da58752a2fa7b604d97b0f6819976b0cd",
    "owns_source": "6a89444dd116f019fabd7ef2465b5ccc9d0b404adbf783dc47d47ad150ebe1b2",
    "target_is_local_head": "a66a280165f7945efa150447183cb391824878b3d002c7cd7d964f60d8a44096",
    "advance_target_cursor": "b16e6854e6b213b04e89a740f8b20f1c00f13ae88e8da38a39faac55ec6b0c26",
    "handoff_applied_height_to_durable_reconstruction": (
        "e78d702c927524d363d59d1a098bfd6649d6d399e3f0252faac9d500c77b5a80"
    ),
    "target_is_global_head": (
        "f71465dbc379cc235d6669a17c7cc6eca1b456acbf5b8b996ab39ebebbac05ee"
    ),
    "next_schedulable_target": (
        "46a5fc46be8b95eb70516a03c7fc52537238a7fe26545540a1a85413e3465475"
    ),
    "advance_after_attempt": (
        "e678eb75bf1e124b8c3ac7b196bc9abee8d5429a6f064a1aaf64851291bc0e07"
    ),
    "drive_with_budget_ack_and_durable_history": (
        "334da253eab1b11913ae2f909d162c31f8e2022c4f11fa3aec141b3644a44d52"
    ),
    "drive_bounded_with_ack": (
        "3576ef1066ef4460f076423c8f360b0f7e232fb75d281965b4e94ccc68f37726"
    ),
    "applied_height_reconstruction_covers": "8077bb6f4a3806d952538f2d7f80ba2919523526e678bff8258f81eb6ec85515",
    "durable_exact_output_handoff_owner_pair": (
        "f848a53734e173e64122e3cf198d6e0adfea99909594e667c56a7e11ea271a84"
    ),
    "certified_sidecar_prefix_covers_occurrence": (
        "019ae94e6c1ebdea5e6948361cf117d49003d4433cd9daa7c3a28f2e1e391741"
    ),
    "handoff_applied_height_output_to_durable_reconstruction": (
        "7444ffcb40c52af925865f03520de3eac64f59d1c31ae6e549c5bf92e17aaf84"
    ),
}

_APPLIED_HEIGHT_TICKETLESS_FINALITY_STRUCT_SHA256 = {
    "PendingExactOutput": (
        "cb82c98b833dc4d5fe13de6b481fbff930721aa248aef01c941b7f0d6ac16b9c"
    ),
}

_APPLIED_HEIGHT_TICKETLESS_FINALITY_REGRESSION_TEST_SHA256 = {
    "production_exact_output_observes_finality_only_after_state_commit": (
        "fb1b0dbf139f30e926adf2508581e57c601d6b029a730c4fb6d4207bb5bb24b6"
    ),
    "applied_height_finality_releases_only_ticketless_global_topology_target": (
        "5ea46404af59fe56dcb8c47e5ac672fc40f8ed1ee66f686c20cebae4f93e69c9"
    ),
    "applied_height_finality_releases_only_covered_ticketless_payload_chunks": (
        "570503af2578b9f795edf23d86722b64a338d788a00e07e89b4440e0bbfb38ab"
    ),
}

_RECOVERED_DECISION_FETCH_REFANOUT_ITEM_SHA256 = {
    "rotating_current_archive_targets": (
        "8f86e1b15df13eded32cf9d194c2511759806891c3acadb8617f2c1d33bdd78d"
    ),
    "ProductionV2Services::current_archive_targets_with_frozen_fallback": (
        "f9f649ecdbd11566ca1f6ae219ad6263ae439cfff3fd6eb25711a7f6839a9a3f"
    ),
    "ProductionV2Services::recovered_decision_fetch_fanout": (
        "a415ee3901c2a8e576157e8716e6b80953589252912117cbd1b1dbb3b7d19338"
    ),
    "NetworkBaseHandle::configured_peer_ids_bounded": (
        "dcb60146ab97d3c71922b20b03f7a8e9a9884afed53ea8923c4bbb435f768aaf"
    ),
}

_RECOVERED_DECISION_FETCH_REFANOUT_REGRESSION_TEST_SHA256 = {
    "recovered_decision_fetch_refanout_reaches_live_peer_after_topology_ticket_cancellation": (
        "17d64de7df3878996b2d18decd8bb1dbdbf934095bf3e3db3d7abadcb12ce117"
    ),
}

_RECOVERED_DECISION_FETCH_REFANOUT_P2P_TEST_CAPABILITY_SHA256 = {
    "NetworkActorAdmissionTicketTestFixture": (
        "b880a256bc9dfdc4dc28cd5dfcaab6bebdd0d1a2d404861d6c898c809c9bf51b"
    ),
    "NetworkActorAdmissionTicketTestFixture::for_topology": (
        "84f309e32cb1359903512b07ba6b523ef82889479b014fcd5f3e4a92300f177e"
    ),
    "NetworkActorAdmissionTicketTestFixture::cancel_topology_membership": (
        "8f905cc135f577bde482844c2ee644748a113e3335af293cd8700b51f8361901"
    ),
    "ConfiguredPeerSnapshotTestFixture": (
        "d5ef53a2a466b1fb802b4d4dd3ad3c2dc6c0b3259384b132df039e4a93b83979"
    ),
    "ConfiguredPeerSnapshotTestFixture::replace": (
        "3cb38c9d29dabb11083c53e05fbe6219a739594fa07e231ec0e2b9742117b559"
    ),
    "NetworkBaseHandle::closed_for_tests_with_configured_peer_snapshot": (
        "9531a182704338776b9e63d5bf6ccc534f370982cd0ac8d2944e6782a86b3168"
    ),
}

_STABLE_LIVENESS_REPAIR_ITEM_SHA256 = {
    "V2LaneWorkAdapter::retire_historical_recovery_request": (
        "fb3647a2310d0d73307478c173431884f14d9dd7c67df9ec84b7b6beb0f65aca"
    ),
    "V2LaneWorkAdapter::schedule_historical_recovery_request": (
        "9f93740ea85f6c7d320b25b97b182aa4fd19af763837dbe64473feed3ef89722"
    ),
    "V2LaneWorkAdapter::accept_historical_recovery_response": (
        "56b8cb036dc038f74c2a6f92ad4ebd24016415182e3d26608cb6b4cf7bb02102"
    ),
    "V2LaneWorkAdapter::authenticates_certified_merge_sidecar_service_for_requester": (
        "4121046358ab568ad4f9730de806534e17b48aeec63541f123b61b60c88116db"
    ),
    "CanonicalExecutedBlockRecovery::service_next_with_archive_targets": (
        "fa2727f0283f2e6996d384c42684d79a1cf5d88bde42d4b8cf36945fa6af96e8"
    ),
    "runner::service_canonical_executed_block_recovery": (
        "ae6d4f5f57a8d85547e33f2b13754a5a7cfe42472aa04624187ee8e568c24813"
    ),
    "PendingExactOutput::drive_with_budget_ack_and_durable_history": (
        "334da253eab1b11913ae2f909d162c31f8e2022c4f11fa3aec141b3644a44d52"
    ),
    "ProductionV2Services::drive_pending_exact_output": (
        "2e2cfd4ff54577ecd72c948271cc492865c1ad00bb7e9675885d2d151261f010"
    ),
    "ProductionV2Services::schedule_released_kura_replica_advert_heights": (
        "befe1e00d8ae9784ca6932b9b416f67170c11b05cdde2f40e60cb4a49fb4504c"
    ),
    "ProductionV2Services::retry_pending_exact_output": (
        "0f86ca2f246cd92b34ea9b7950815b2f18fdf0e9aa2e25379703ca51e97b72d6"
    ),
    "ProductionV2Services::enqueue_exact_fanout_while_guarded": (
        "a76f7c05dfbafb57b0b58f4d714936eda47d16a17dbbc7be5d0db88ee399de9f"
    ),
    "ProductionV2Services::enqueue_exact_fanout_while_guarded_collecting_released_adverts": (
        "a4175b7fa12a3b3725baaff7f93af2865208fd47976ae3a6476f78b7631e3ddc"
    ),
    "ProductionV2Services::service_kura_replica_advert_refresh_turn": (
        "7a933f63c09158be70133d82ba63e5c877fb46a94f1ff4ce17e9e6b2f83349ab"
    ),
    "AutonomousLaneReservationSlotPlanError::is_retryable_after_state_or_kura_progress": (
        "853209983b3c0f061ac16fec1d1b09f21474768e1b027d0c103d9057d1f326f5"
    ),
    "V2LaneWorkAdapter::schedule_autonomous_lane_production": (
        "57901aca3a29127de1b1f156bf418eb22cef615682a9543c5a521d175d4d713c"
    ),
}

_STABLE_LIVENESS_REPAIR_REGRESSION_SHA256 = {
    "historical_canonical_body_retry_retains_prior_archive_across_rotation": (
        "dca03c61d212c8cae11ba82c44c237bd3b0c2c631e51ae445be9d887cd62f204"
    ),
    "disjoint_current_roster_requester_receives_exact_historical_sidecar_chunk": (
        "5a3b7fe6fc22bd1c889f9c0751913b74a7a32d93e2131e90a8f590b761548340"
    ),
    "canonical_executed_block_multichunk_pins_archive_and_refreshes_after_poison": (
        "43632c34ffa281a86ad4a0de1afd7000031f4aa1d9b0e09eef024c588e9bcdeb"
    ),
    "terminal_retry_revalidates_only_ticketless_exact_kura_advert": (
        "b0c6484436ecec1931eaa4ad8886fbb4cc0373fdcaf2e092fc33bd3a7ed72cd4"
    ),
    "terminal_retry_revalidates_only_ticketless_exact_kura_queue_plan_admission": (
        "7c7e4924a405f79477ed75221a6d6a9c7d62d5c525117cb4ea38365d832e5f22"
    ),
    "autonomous_reservation_retries_only_transient_planning_failures": (
        "fabd75d09cf3453ffe731485ca2b06034d333ee084e4e73704242d8c75184a4f"
    ),
}

_APPLIED_HEIGHT_PREDECESSOR_DURABILITY_HANDOFF_TEST_SHA256 = {
    "applied_height_handoff_retires_only_exact_same_finality_nonwinning_autonomous_outputs_atomically": (
        "6180ebcb3f417cf30d175305545bb980449b99f50bcf74ec300198e15bad5881"
    ),
    "applied_height_handoff_accepts_kura_applied_ordinary_historical_lane_output": (
        "63da596c965313f6d1670a4dde34f5b4886f7dd8983ac32d8a969f57636d869f"
    ),
    "applied_height_handoff_accepts_record_backed_autonomous_historical_lane_certificate": (
        "64d78dacb9ebb2a99b8d48aff8202bb62aedd080450360bcbccc1d57e9c9cba3"
    ),
}

# Complete token seals for the source-isolated exact-output corridor.  These
# complement the projection checks below: a mutation cannot preserve one
# positive subsequence while inserting an early return, dropping a sibling
# source, or disconnecting a writer-flush receipt elsewhere in the same item.
_PRODUCTION_MERGE_SIDECAR_SEAM_ITEM_SHA256 = {
    "CertifiedMergeSidecarChunkAdmission::from_admitted_reply": "a2b593a56511c97eae5bfaca2235490ace31a128884729704bb6dc907a98da44",
    "apply_reliable_flush_application": "93912d6c690daa60be7fda65daa0a997a0e55839d3661560ada00aaca8dd8a52",
    "MergeSidecarTransport::with_limits_and_server_stream_capacity": "acdb21ade067f5460c4019cd4fb0f65741d2f2707ccd1ac447d260641dc3b7f1",
    "MergeSidecarTransport::derive_server_request_capacities": "02926b83f40ccd433a5d18f79b4cf725d81a81d198d0b5e4ac043f09354f52a2",
    "MergeSidecarTransport::begin_request_or_close": "8fae96ccb47e45e090ba0ddad6359153951f47583e23be35c6eccfa6ce61e3ca",
    "MergeSidecarTransport::release_unsent_request": "dab1c81134717c57640cd64da49a68ab99cf61d474e02d7b4f1c0ee0b398793a",
    "MergeSidecarTransport::release_authorized_server_request_attempts": "713f97a0a252983fffcd258d631f1052cd3801f634d8554d1febcdc65cf689d9",
    "MergeSidecarTransport::park_authorized_server_request_attempts": "a7f6ad8f3a595fe655f0535b11d2564a2a783d60df1cc6bebeadb704606e96f6",
    "MergeSidecarTransport::reclaim_inactive_outbound_attempts": "9600b575eb21edcb26955867d706880f5040c062ee84b45c266efeb5d484a1a7",
    "MergeSidecarTransport::prune_server_gates": "13f9e547c581b576aed5a552fcd2ecd2f0ea7260fe5a72c7bd7a78832ceb61c2",
    "MergeSidecarTransport::server_request_source": "c1c1f62439ac7e654a67db7cd7de13504315c3da463da0ac7d639e8c3722bffd",
    "MergeSidecarTransport::source_gate_count": "61c262742b3758298046503a720335804b1981748c8bc156527dce9705c80bc8",
    "MergeSidecarTransport::server_gate_attempt_count": "e9c80512a68ae8368554aa4e6990a625ca57a5b6c5fb47ecb5b7bf1598a38acb",
    "MergeSidecarTransport::outbound_attempt_count": "d0b5ac92c19eab61d75f4b461c6ecb4a64d6861fd5baf194a52737bf4843879f",
    "MergeSidecarTransport::source_outbound_count": "3b17f1f9955d281381cdf58b38f910687a62423dc48f8ddfa96fc150d401f451",
    "MergeSidecarTransport::global_outbound_bytes": "1ec3ffa56334677b60c9c920e8e15a562ed04653674849e665b2d44c623c73a6",
    "MergeSidecarTransport::source_outbound_bytes": "2c4678e27b465dd69d9b36e07d9f8b1e85b56f2568282a90f9bbef47a919c3c7",
    "MergeSidecarTransport::route_update": "2f8d4bf918efcb9070aedbb7e4d57baa89f8127ebcf4375c53af35afa852856a",
    "MergeSidecarTransport::alternate_source_is_authorized": "8ccfab565eb86d3527950b783f0ef326303e0a386dc10f268e25f72fcee25421",
    "MergeSidecarTransport::route_source_capacity": "162b152d56446bffa077c6005f4457ce2757a26a73306018a80cc53f68a9ba25",
    "MergeSidecarTransport::can_add_outbound_attempt": "7ec9222597fcb0ada3b06a589d775066cd94335c8d6bb2cfcb2f685fb922c43d",
    "MergeSidecarTransport::next_server_request_materialization": "445d03272e85e2bf3f7e3f054c2acfa2d8ff6434b86c62defca965bc42bc3002",
    "MergeSidecarTransport::admission_after_fair_materialization_selection": "26e50ac363ea2e8c75497ad2df045ab1dcd4ed2315db16147cd9488be04d82e9",
    "MergeSidecarTransport::admit_server_request": "78fcf00b793e0ef8b871657c83c3b05b8b81149e789470faca831fd8c3b50fc8",
    "MergeSidecarTransport::cancel_unmaterialized_server_request": "40f0831462c1e44e4e55919d9ba9dfc443b6901dd963c47ad79106ce274586fd",
    "MergeSidecarTransport::enqueue_response": "2e11a16de83ba8372b3ad92e58e6b222f9268d70611be2bb1c56f765f4aaa06b",
    "MergeSidecarTransport::drain_outbound_chunks_durable": "b15f79eea3fa0a065bed3026d821e05a7562e379e24dff34db19c8b32b601ecd",
    "MergeSidecarTransport::acknowledge_outbound_chunk": "9f9a801118e363d0d821ab528855b60354a0eeaf3acd186191d7da4512155484",
    "MergeSidecarTransport::tick_bounded": "8a01af0d92b9474c1b9c4ff0c861d2b29315b4681062e1db668a4b1db543c541",
}

# Complete seals for the crash-safe semantic request lifecycle. Process-local
# reply capabilities are deliberately absent: only stable peer ownership,
# sequence floors, terminal/pending cursors, and immutable pending-chunk
# identities may survive a restart.
_PRODUCTION_MERGE_SIDECAR_LIFECYCLE_ITEM_SHA256 = {
    "RequestStreamState::allocate": "035e187a842019c9adf402f2984a0e69ea7de331056e6038d0f3974311e566ee",
    "RequestStreamState::close": "ea56cf4ad45b41875017f40148321ce2189dc77919fd6dbd60ba83b45107166f",
    "RequestStreamState::emit_close": "662b1d15ddcaa2ba9b0ef4c776e7d30647f5aab383ffddeb5bf6dd567178a552",
    "RequestStreamState::acknowledge_close": "4a2e627dd28de3c332a45c68a351d86bb392ebf07a9c4a4fa8021399bfb78dd3",
    "ServerRequestSource::budget_source": "ff85c7db75fcbff14abacdbd2d23351e5c244b8046989f7ed6d8feb2bfe59663",
    "ServerRequestSource::shares_budget_with": "d2fccc4fab299dac39f9a6bac5639887a4dd6e115eae29ec90579da93c75051e",
    "CertifiedMergeSidecarGenerationHintV1::canonical_hint_id": "2731d88621ae21e7119162b150578b5419f3b1a7e04676f9565e6d2f78a0ebe7",
    "MergeSidecarLifecycleSnapshotV3::new": "b76fc1b3f9d52cd47e221b55ca35cd814bf86290cac86b7de8c8150dff0d8754",
    "MergeSidecarLifecycleSnapshotV3::integrity_is_valid": "413dd7181f9898e41bb9b9f2bb826ceaa793c44f9a4a12943d36fe58ae32cf0c",
    "MergeSidecarLifecycleRootHighWaterV3::bootstrap": "09c144912ad447177997f12fc31de749830f39e500a726e7ca7d0490ccf0de51",
    "MergeSidecarLifecycleRootHighWaterV3::new": "6fb62886f1fc7515ccb93708a257239f55e2fcb252543fc170427cd2ed4944fe",
    "MergeSidecarLifecycleRootHighWaterV3::is_bootstrap": "af4f8d7064a8e7d4334a2fd28b6979ce8f97040f9a8fcdf79840e823b6e41621",
    "MergeSidecarLifecycleRootHighWaterV3::matches": "3138b2e1831a9710b7dcd84570fdeceaef945398cb0bb1e16ba7ff585a431d1f",
    "MergeSidecarLifecycleJournal::open": "81f2b35d6df08c5cac28ebb95fb240a7e5a949e52283c7f4ea5ccd8a357fdd33",
    "MergeSidecarLifecycleJournal::state_path": "e1a8db899e4e4ae845a2a5d5c7731c3ec7a4d04a57494a90876c761063b14789",
    "MergeSidecarLifecycleJournal::state_path_for_generation": "958b5b38f871dc10d788964dd806276f7112376c426412c402ff66d2086acf48",
    "MergeSidecarLifecycleJournal::temp_path": "4023b2cff04791329db79483fe6ac8dd2af85f90ded291d23c7cbbb33c17bf7a",
    "MergeSidecarLifecycleJournal::root_high_water_path": "fc93b03a37534d588c93fa488d50dad2909c133463a2d11cf0176bf42c8e05ad",
    "MergeSidecarLifecycleJournal::root_high_water_temp_path": "775a520e25b9c83c46aa08944341590719b4d59d0d2d12af95fb9d2c968099d8",
    "MergeSidecarLifecycleJournal::publish_bootstrap_marker": "40c9b879e62f60d03b3a81db96b1c67a9f97fefb36fe5453e7cc44ecdf836c92",
    "MergeSidecarLifecycleJournal::bootstrap_candidate": "5b0488a94395189e31943072b55da8dc71da1dda18106afeced87f73ed7c1048",
    "MergeSidecarLifecycleJournal::finalize_validated_open": "86894b3b15406d99916d483471c8ebb16ca3234745e93335768d100fd0bd1b0c",
    "MergeSidecarLifecycleJournal::sync_directory": "6bebdd14196431dd0c8c9160bca5965b8c902ae8fd9addd79596e6ff65ca9d8b",
    "MergeSidecarLifecycleJournal::artifact_exists": "f975c5a57d4c64323fa39d6fc2439658d13d3db6b944ad62805feadd841f7edb",
    "MergeSidecarLifecycleJournal::reject_artifact_if_present": "0d3908089c208e1abb49f232c1191b354960c100f1b3ce7dafceda699adfd7f6",
    "MergeSidecarLifecycleJournal::reject_known_temps": "3e359c0b5dc40cbb9e51b962ba8c840c9dd9837bca38378b58d38c95346ee06b",
    "MergeSidecarLifecycleJournal::remove_regular_artifact": "91167e9513b22b8b66d14f6fa3891510e4dcf88964f1ddaad777e6c69d76b911",
    "MergeSidecarLifecycleJournal::validate_regular_artifact_if_present": "4ab43f67d2bc6776e74b901e5cae02dd27c4259e4b5e525d86f71832893d2528",
    "MergeSidecarLifecycleJournal::validate_known_temps": "79f498aac06957b42a448cb12169f0e88612792854cf870e9065fc1501b14044",
    "MergeSidecarLifecycleJournal::discard_uncommitted_temps": "5075863b4359aff4aca8f37895fa497cfbcf82298ddef44e5d439a92e8a337f9",
    "MergeSidecarLifecycleJournal::discard_inactive_slot": "675c63c3195613a66fd9551590ebedd83e2f58527f13a0ceb04aab1f757926e9",
    "MergeSidecarLifecycleJournal::validate_directory_entries": "2c13ac3a3cec9f55d88eecf2f7781635f70fd70c824f71750e35332f6b9d2cec",
    "MergeSidecarLifecycleJournal::read_bounded_regular": "38f7bc44bed7b7c1f1d9847e75f65430dc6bd15532827ce888c0c8ba824e0563",
    "MergeSidecarLifecycleJournal::decode_snapshot": "2473d8b7f9040a2a041bbc46f9c23a8f8d1940ebc779a455edf63590f5a1c402",
    "MergeSidecarLifecycleJournal::decode_root_high_water": "76f8a0caa1f65018f82353e36d2bb07846cb8a49583aac60483dd7eab0c818c7",
    "MergeSidecarLifecycleJournal::load_pair_strict": "c3e273a90e4d180f08476d9bf7dd21b2ce91f0032b227857c09fbbc9b119cc5d",
    "MergeSidecarLifecycleJournal::load": "d7c2d8180e6579b1b8213fddf5ffcb984ef7025a52e66fa5ed4bc2d26c6bf6c7",
    "MergeSidecarLifecycleJournal::write_new_synced": "ab267b2611abb24926fad0ad306da0cb44a49eec565cd7450c76f762473fb2fa",
    "MergeSidecarLifecycleJournal::persist_atomic_replacement": "e8a844216e2110bf463f9884e7bff9d06b8dd0a9d6baff8cf1835bbb29798064",
    "MergeSidecarLifecycleJournal::live_generation": "8d2d97c739d2a32c7e899751791a8ef9528bcd6a5a1900a0f0f7e017a27a2fde",
    "MergeSidecarLifecycleJournal::preflight_next_commit": "5c58aa339bb87024e46da01db648c75a7da2e3e78f12fa7a285414ba70e103f0",
    "MergeSidecarLifecycleJournal::persist_next": "e4b9c82c810f07d1158140e054ffc8e3f07ed143b4e0d68f17e0f3d136c541e6",
    "MergeSidecarTransport::lifecycle_runtime_geometry_v3": "34f7fde0aec9a4d933b9bfd71afa80c9391911797b2db30074f01db8110d119f",
    "MergeSidecarTransport::lifecycle_geometry": "eae65fb2d75f710c039fec57b4b4cfa0801b6620393a1ce90958c6656cd6496b",
    "MergeSidecarTransport::lifecycle_geometry_for_server_roster": "4b6d81251a26903563d0d5a7b309e2b91ebc16fde3e9d78cbae72953d9694d45",
    "MergeSidecarTransport::lifecycle_max_snapshot_bytes_for_attempt_capacity": "6a8066cd1ab881b346c2f410cdcb3fef9001b787b2016de257df86457d9cd3f9",
    "MergeSidecarTransport::lifecycle_protocol_max_snapshot_bytes": "2ab3af73ff8454c4a11044f0b87d81e3484433de497d2c2f14392bd3c63b3a52",
    "MergeSidecarTransport::lifecycle_snapshot": "e51da54b26a259562e37b630f1a3c5e6f6ee6c3dffff2824557fc16801f4e6b5",
    "MergeSidecarTransport::restore_lifecycle_snapshot": "8627186b8d588686e3cb570af7f57cc72ee6781910adfb59c0c5de37b41d47b2",
    "MergeSidecarTransport::configure_prior_lifecycle_server_geometry": "ac39616a1ae23408941413d2472553d6234589168219cf31c0a47b7aed7dcfcd",
    "MergeSidecarTransport::open_durable_with_server_stream_capacity": "3e2ed1cb57775d36918a553dc3e17e065a4e994c87d1e67ddfef2ab3b8344a0a",
    "MergeSidecarTransport::persist_lifecycle_projection": "721b44da8a5911f197ad7326d8cfcb7919c8df7bbd1ee4413af1817f5b03eb11",
    "MergeSidecarTransport::preflight_lifecycle_mutation": "e792b37472d616f160d5dd5d2c28f869d35670df261e31ede7e56d8b93b55030",
    "MergeSidecarTransport::persist_lifecycle_state": "4a8658892bd135d9065f5f8aee9ad6beb73f7f3993b54dc3cb1e7c236bd5fd2b",
    "MergeSidecarTransport::rehydrate_with_exact_geometry": "95238c6a27265eaf9030443a5c92afdbbbe071ebf1337684f3b8f4b88b25aea2",
    "MergeSidecarTransport::rehydrate_with_exact_geometry_after_durable_handoff": "d5c8d8cc61fbe16f2ad8b91595ed63f6ca477e0c8b5269be6417ad486472e821",
    "MergeSidecarTransport::rehydrate_after_lifecycle_restore": "221f0ed6a140103b37b857a210da74887d77aab9a19400ef96b4f4993c5a420c",
    "MergeSidecarTransport::validate_retained_height_geometry": "a52a9db9ba2745b1555ff5422edc3c64fcbb3b910ae5f62216009866b706b923",
    "MergeSidecarTransport::requeue_retained_outbound_after_height_rollover": "a09ba68802f834de1a13219085ee6f24e7d1bca666a4c951b80501489eed0be8",
    "MergeSidecarTransport::allocate_request_sequence": "4a00c1f454545b38973384a4a4e968d8eb725f80b9d5c9b54b03afed0327c010",
    "MergeSidecarTransport::close_request_sequence": "d13ef957882ac705a2fdb715f72faf705c2d6c33248b354b61b834f82ce5ffb5",
    "MergeSidecarTransport::begin_close": "023c75991bc11e8528e4692fb11101e1e565f18f496be829eb20a4935f13a9f0",
    "MergeSidecarTransport::begin_request": "1e337d236787c31def6238bef35a2e461abcc78e70b47a54347cb7948e29195b",
    "CertifiedMergeSidecarClosedPrefix::covers": "21db3c4644755e3724626fc5c862b41346facb61576c135749144bd17ba803a6",
    "MergeSidecarTransport::acknowledge_close": "a6dc61f720126bf504dba5bc556385739819b18d6d9af845034b7e175d1ad0c5",
    "MergeSidecarTransport::acknowledge_generation_hint": "86b52bcc4c6063ea5ddf2ce983bd204784d5c129843e5af9f9db50c1d3cd4efd",
    "MergeSidecarTransport::generation_hint_post": "8f16175266e8ef580a910ddb39208d1963c471d058f164222428a90bf78eb922",
    "MergeSidecarTransport::preflight_server_request_stream": "039e4f6bf2d7ed58244185b7a5811f40b19737e953a603eed417f960546008c0",
    "MergeSidecarTransport::record_server_closure": "a8027bf18c82e939aed4072172cfe52b3c3116d5325a3f08c0b611ef95ffe6d9",
    "MergeSidecarTransport::server_generation_is_terminal": "91daaff340270d5a630e818719ce26a4b02d6264e80c4d4f2114bb2eea378f38",
    "MergeSidecarTransport::transition_server_service_generation": "442b14db6826182fa67c8563a3dd7412e7fd2bcc8824b8575640512547665675",
    "MergeSidecarTransport::transition_server_service_generation_after_durable_handoff": "7bf098b2a24c176a5bdfeb473f860986a3219675d10428be7550af1fa249a797",
    "MergeSidecarTransport::transition_server_service_generation_after_exact_output_fence": "68251fab01ae053907984e39eeac0779f1018d50cc069e791a8a85fe010e369a",
    "MergeSidecarTransport::prepare_server_service_generation_transition": "ae7d51586773145d9933369af877e09257c66def9ba4300d4c251109cdc0dabb",
    "MergeSidecarTransport::commit_server_service_generation_transition": "bafcbce053766a2f838c437d8c4f55cbbae22fe22f91e0e5a94944a707146684",
    "MergeSidecarTransport::ensure_server_stream_slot": "54412c913251408bb8da2595fb3fc36ef3b7df4486f09e8c85f8ab821f887597",
    "MergeSidecarTransport::supersede_server_stream": "4bfce430840bb6fd125779e211580bf98b60358736e735d2d124e8726d8a8bc1",
    "MergeSidecarTransport::advance_server_close_floor": "a8b0fe4fd6c15a17c344f1a318e39421efffaf3c24693e1965444d7c4ccb78b9",
    "MergeSidecarTransport::admit_server_close": "b4f0563eda7a12f58f3715b84f68ded706d03274c1744756ce8ba56a3cb2f073",
    "MergeSidecarTransport::drain_closed_server_prefixes": "517b76f57cca2c96ed8d1fbd53db8318b4003e09c7401dba9c8478ca7217f1c0",
    "MergeSidecarTransport::confirm_closed_server_prefix_handoff": "76c4d49654aebfb34456efc2fe5f0ac1737226a954bf3916bffe2ca5319ad7b7",
    "MergeSidecarTransport::server_gate_attempt_count_after_close": "d10ba051868323f576c5030b9d9c73c4c8debe4ae7bb0da21a04db9864e449f1",
    "MergeSidecarTransport::server_gate_count_after_close": "7b3476b7ec3e31013a84f9c331795d6c1218d03ad614734c2e17003e554baa0a",
    "MergeSidecarTransport::source_gate_count_after_close": "a065e18f460113a9ca617675037064f607d592e4edd456744a5ea8a2bcc97a24",
    "MergeSidecarTransport::drain_outbound_chunks_inner": "999e7704f60196377c7e7ca0c1f380b0e774d889ddf4ae34b2bfa084b96fcfee",
    "MergeSidecarTransport::finish_completed": "f3bfa218ed218197e1450d68ca7538c3dbc45c1ea5e2819035c49baaff3b3d62",
    "MergeSidecarTransport::discard_invalid": "d63d81f340e4309f0b795a5901f66e71ee0cb315910980012db0ec0f91b4e254",
    "MergeSidecarTransport::retain_pending_blocks": "ea89164562a8fa62883f5cf48e6938c63bb45e69b124ee4943826bad6001f45f",
}

# Attribute-inclusive seals for every platform branch which establishes the
# directory/file identity used by the crash-safe lifecycle journal.  These
# helpers are part of the trust boundary: validating a path and then reopening
# it without `NOFOLLOW` or without a stable handle/path identity comparison
# would reintroduce a TOCTOU decode path.
_PRODUCTION_MERGE_SIDECAR_ARTIFACT_HELPER_SHA256 = {
    "lifecycle_artifact_identity": (
        "301e701d0753691df10a350b69a5eea47cdde38195e156c8b6dbaa5e72aa0d61",
        "9165ef4e4c97ef04d2ae55b5b00605853a2db11688432cbed96a61f567f3a86a",
        "ee13fb0e994aedad25ddab6ead76f0cf992219ab68beffb348f51b1c532bddaa",
    ),
    "lifecycle_artifact_revision": (
        "119f0656e2127e4c66bc6915ad8df49d2710006aad9744122cb17ead34b51e42",
        "9bea5709b4c9f9fb0f06d2003e3a26f696af5bf8cfc51b590a842e113cd0956c",
        "f3123c09a7b193028dbafb465fe169b2bf55d1e00fdba6f185a28d5c33b3b097",
    ),
    "lifecycle_artifact_identity_available": (
        "6b621ac96e7d735fdf061ca29f91228fa68e94eeeeb07f5a8205f56e7f48b225",
        "c21870b387d5cd786c93f51f3873466a002cd6dc9d532f3c950e7b53e44d4bdc",
        "f8d258b48fdda51575891c262d03635f0d8026f500740e5c03a24e67012e35d8",
    ),
    "lifecycle_artifact_is_single_link": (
        "91a822912011db14fdf25c7d86a33d69e72e2e25daf250d3513a3bc1252a0b82",
    ),
    "lifecycle_artifact_is_reparse_point": (
        "e0f9e851429da4dfe4f5b2691083c78fbf53272129be634fe06ddd64cbaf9514",
        "f3613fe601a454d7c1a960b49b86565671cec04b4c6dff410272ed37b722c2c5",
    ),
    "lifecycle_artifact_metadata_unchanged": (
        "2ea5f87b8ceb44ae3ef92320ec86c3c6b88b66d49bbb1dffc9574077189641db",
    ),
    "verify_open_lifecycle_directory": (
        "98f56261beb3cd37739864c2f0223e479177000fa6581bfde5cc85e2418e9da1",
    ),
    "open_lifecycle_directory": (
        "1c480a6f24d1d39a6fe9c7c37bb9cfc738cad69609082b66c60531f141f3fdec",
        "cfc8db5f4cd9eaee8a95a476440bb53c2f39423b12860d444ffd713c051f1ceb",
    ),
    "verify_open_lifecycle_regular": (
        "b3354225cd5d701857ffc3ddd06b958ceffeb4b3f611d5bb7af9980b7f52d12f",
    ),
    "open_lifecycle_regular": (
        "1762c07b05b671e00fa3b476b61267510a4313aac498d6f440f4b29651e3836e",
        "536e5ed345f17fe72ce9856541070e24c9cfad92ae8da06e8ef18c4024c54c2e",
    ),
}

_PRODUCTION_MERGE_SIDECAR_BOUNDARY_TEST_SHA256 = {
    "authenticated_source_quota_rejects_origin_churn_and_preserves_other_source": (
        "177b9dcad1c91ebd413bc7824e2a2340c20287fa89d1899f83df154146394233"
    ),
    "legacy_lifecycle_v1_snapshot_is_rejected_without_migration": (
        "c100cca083e1def146023e86038c60b04ae9a641a2e29dad0a7f2926e7232e83"
    ),
    "durable_responder_restart_preserves_same_hub_gate_budget": (
        "2c98602d0876f99438260e9accd99459d09334d014e6707e497cbdf7bdbe3ca6"
    ),
    "fifth_gate_from_one_hub_is_rejected_while_another_hub_progresses": (
        "72591d58cb56aa08d3ea0bce920f7cf36c9a00f6b14593bc5dbf17f3834da444"
    ),
    "quiescent_multi_source_pressure_never_rolls_or_bypasses_source_caps": (
        "e5e5d27f264d8c30229eb80df95e7c95329c5fb42eb3370f0b3a4ddb1bd37342"
    ),
    "durable_lifecycle_v3_root_high_water_is_exact_monotonic_and_noop_stable": (
        "34edbc3a0e29cb88e9c672095506bd8e40aaeedeaf0faa63991baee4aa33c4d1"
    ),
    "durable_lifecycle_v3_bootstrap_recovers_first_commit_and_rejects_missing_roots": (
        "e01fd4052a5a78e492624de151d297dfaf91f44f47894a7645f355e253bf3f48"
    ),
    "durable_lifecycle_v3_rejects_crossed_bootstrap_and_committed_root_shapes": (
        "6288640952d9ea0e72797895aa2de7dd62186daf81da33497bd4bd5a89518c83"
    ),
    "durable_lifecycle_v3_recovers_regular_temps_and_rejects_unsafe_artifacts": (
        "cd06d9140fd9f052787048d9719feba1255e670d4eb0fa619aee24ced110be00"
    ),
    "durable_lifecycle_v3_validates_semantics_before_retiring_crash_artifacts": (
        "b9802b3eb8ebb243b566f6b8fad320369912c600111d3c045f36b9f1674cfd49"
    ),
    "durable_lifecycle_v3_rejects_split_generations_and_rehashed_state": (
        "bee7172a2a75e10ae2d34e2da9ebc8f688bf152cb9c74de324cd38bf9cf0ef10"
    ),
    "durable_lifecycle_v3_generation_exhaustion_precedes_close_mutation": (
        "529af4e365c92c84a976a06f6a1271bb41afc5878a4206bab39bfef06726b908"
    ),
    "durable_lifecycle_v3_generation_exhaustion_precedes_writer_flush_cas": (
        "22bd6d8a616406275daf917c063d1af71d5196ee9953320f4c6615873f8e85d8"
    ),
    "durable_lifecycle_v3_recovers_predecessor_before_state_directory_sync": (
        "6c51610572f862a27b7c4cdb704fd85b9cada24ea849313a61078091782fb518"
    ),
    "durable_lifecycle_v3_recovers_predecessor_between_state_and_root_publication": (
        "37fdf12044f2511e5125de87e8d468d61831be1328ca1778b4737e0518b38920"
    ),
    "durable_lifecycle_v3_resyncs_replaced_root_before_predecessor_cleanup": (
        "d8ac3e60c1d433064f25b8d75226080e3cfe98b0f955de14d29ec769505a631b"
    ),
    "durable_lifecycle_v3_recovers_successor_after_root_publication": (
        "7664893351c81f1a06df874c5a133539e6aab8f8d5ea95158419e2f8c8caa7c1"
    ),
    "durable_lifecycle_v3_rejects_missing_state_with_surviving_root_high_water": (
        "7da31a3cbc8c7d50c7cbb5015a1691b73827385de9598e4fa8cc5db8a53c7295"
    ),
}

_PRODUCTION_LANE_LIMITS_CONSTRUCTOR_TOKENS = """
Self {
    session_capacity,
    body_buckets_per_session,
    native_session_capacity: session_capacity,
    native_body_buckets_per_session: body_buckets_per_session,
    native_source_capacity: body_buckets_per_session,
    effect_capacity,
    relay_capacity,
    merge_capacity,
    native_request_capacity,
    reply_source_capacity,
    merge_share_frame_capacity,
    historical_recovery_response_frame_capacity,
    authenticated_merge_qc_capacity,
    merge_leader_body_frame_headroom_bytes,
    autonomous_carrier_headroom_bytes,
    autonomous_producer_recheck,
    historical_recovery_retry_floor,
    historical_recovery_retry_ceiling,
    historical_recovery_stuck_attempts,
    historical_recovery_retry_tier_attempts,
    historical_recovery_max_retry_tier,
    sidecar_service_burst,
    merge_sidecar_limits,
    merge_signing_guard_limits,
    native_amx_signing_guard_limits,
}
"""
_PRODUCTION_RUNNER_SOURCE_RETAINED_DISPATCH_TOKENS = """
match dispatch_lane_work_effect_from_snapshot(services, next_effect, queue_plan_sources.as_mut(),)? {
    LaneWorkEffectDispatch::Complete => {
        dispatched = dispatched.saturating_add(1);
    }
    LaneWorkEffectDispatch::SourceRetained(effect) => {
        if effect.retries_from_native_catalog_after_source_retention() {
            continue;
        }
        if !lane_work.requeue_effect(effect) {
            return Err(V2RunnerError::Service(
                "lane-work scheduler could not retain a source-backpressured sidecar effect"
                    .to_owned(),
            ));
        }
    }
}
apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;
"""

_PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256 = {
    "V2LaneWorkLimits::new": "be6dab607a9d6656ec34deb79c2cc0aff0e75d59731c290051e0f6dc10ba6bd5",
    "RetainedMergeSidecars::rehydrate_for_successor": "709b44e4cf845ffe76903ad4d7f61b9fa174ccc1f0a1a793d6733a37a23cd0a6",
    "V2LaneWorkAdapter::new_with_output_guard_and_transport": "017c1afe0515ff169d85bd9fd1a9ba86689ea35405952fe7647c66f42dabc8dd",
    "V2LaneWorkAdapter::new_with_output_guard_and_transport_inner": "2cc9bdb426745cf9db94c9bcec1021f9aa111ce189202a9f6152f73d1e17dc08",
    "V2LaneWorkAdapter::activate_after_lane_drain_queue_install": "638503cfcc9963213cb6146d16d82ab270016e6f5da7bbbc8b2918aea9120cb2",
    "V2LaneWorkAdapter::into_retained_merge_sidecars": "96d1e194eda3660ccccf9b4e1860b26a4db8d9ee9fb2bad4f988e37c85627218",
    "V2LaneWorkAdapter::accept_relay_message": "87420fc8a24b8fb713af40ba5ba2f2df0efa3a04cf2893b087b2f221f34a0603",
    "V2LaneWorkAdapter::accept_certified_merge_sidecar": "8e348b518da26f91fff8840d1e5bf4016d783fef4720bc275baba90ebe14a0bd",
    "V2LaneWorkAdapter::accept_certified_merge_sidecar_request": "7906403df13232aa6cbe853cfec670bea780f091d6887503c4a7420f2195ba92",
    "V2LaneWorkAdapter::accept_certified_merge_sidecar_close": "801722bdc8c28483601a0e35e98ede130f2f767af8cda9c7d423522c8e3a2c01",
    "V2LaneWorkAdapter::apply_closed_server_prefixes": "ca526c1cdcf5ade6263ec12641207c4edf83b5ca71c48a5ed39c8bcdc972a2db",
    "V2LaneWorkAdapter::coalesce_closed_sidecar_prefix": "70aeb1f315515d6c69f913afed4e4874b834d74972c68d1058a8b8916b03e771",
    "V2LaneWorkAdapter::drain_closed_sidecar_prefixes": "2d0a07530ab64b95f687a48fa09cd029a40bad9c18ef29bf82e8b624274bf2e3",
    "V2LaneWorkAdapter::requeue_closed_sidecar_prefixes": "97bdd3f66d265ba00e0cc088af59f005e7cfdc97c138cad9faa64138f944df2d",
    "V2LaneWorkAdapter::confirm_closed_sidecar_prefix_handoff": "7aaca416f7ad687d7ee0c1b99da038d11dbdc710d81229c340a63bda10476c28",
    "V2LaneWorkAdapter::drain_retired_historical_recovery_request_hashes": "58d7eaa56cc67fb2c6febf6daaef8a53b42eafc45f338bc2836b2538077b6f09",
    "V2LaneWorkAdapter::requeue_retired_historical_recovery_request_hashes": "8ed35047dac32886199b4a30367edc60eafe5facb6c94fceeeebfe4902229fee",
    "V2LaneWorkAdapter::retire_inactive_merge_sidecar_requests": "f2e860926dbb7f4c6bcb7662f8a581a99a0520e6481d41b6761c63e758200d31",
    "V2LaneWorkAdapter::drain_retired_merge_sidecar_request_hashes": "5ccfdcee586db0971558c405b6b1e30d825daf994d285be15d1dc599a7ca3d32",
    "V2LaneWorkAdapter::requeue_retired_merge_sidecar_request_hashes": "567b23c40fbe631812a619a94625460b068130b3c9b99368023f30ecdbb2e5bc",
    "V2LaneWorkAdapter::coalesce_acknowledged_merge_sidecar_close": "5520c9386a19d14e3d68e23ecd34398a25d9fb998f17a106c8f0b7cdd788a4df",
    "V2LaneWorkAdapter::retire_acknowledged_merge_sidecar_close": "a1a9b7ac853625af8d5d08375a37ef734d29a26003cc9f46c52d685cd570d9ae",
    "V2LaneWorkAdapter::drain_acknowledged_merge_sidecar_closes": "09f677c45f9d4a8109e3e32c6c50a69684555f60a02cc9786257e71a98baaf87",
    "V2LaneWorkAdapter::requeue_acknowledged_merge_sidecar_closes": "4c55eb447a94ff72777217325e0d02402f234208accf1199571bf7592d7f2e60",
    "V2LaneWorkAdapter::stranded_retryable_sidecar_control_index": "09ed26efa19aefc39d448b1bee81d5b070c272557137e89a51c4f1dc6334419b",
    "V2LaneWorkAdapter::replace_stranded_retryable_sidecar_control": "13094aa7fc37a32648ebf74c461802e857173858f09d7c61ef0054530e340e4c",
    "V2LaneWorkAdapter::service_next_certified_merge_sidecar_materialization": "ac05dcfd3b1457f5632cbd3eb9fc337ee2a1200b6ce02e2cd154c392a3f5cc32",
    "V2LaneWorkAdapter::persist_anchored_sessions": "ac81d09b993cf6c83cfcf157d87de1873395bfb240bdc256159243dc05387ee1",
    "V2LaneWorkAdapter::hydrate_canonical_lane_artifacts": "08ec48a97069a5d90426a973c98d4a4090fc0296f530d5d363453471b2f4e8ad",
    "V2LaneWorkAdapter::next_effect": "62af9ea4c3707845b5b097a27f5cc9281b8ade4bc60db49cdbc9f1c3e2b3496a",
    "V2LaneWorkAdapter::effect_count": "3be06e0c96fdc63e06952ec83b5aa900daf39912955249ca6aad64ec50e1354a",
    "V2LaneWorkAdapter::requeue_effect": "5259377bba158615135666cb3cddf88e0fbfbdb63e55a7691ba397e34195d856",
    "V2LaneWorkAdapter::drain_effects": "478982ec7c7cec9990a70993011e34e0cf79f57fb903b3c7cbabc040052b1aba",
    "V2LaneWorkAdapter::proposal_predecessor_is_ready_for_progress": "af90f5ebe15136bfc8255dc8d1a8aeac7101d4226c79ef8a94e4821eeeb0d78a",
    "V2LaneWorkAdapter::preflight_effect_insertion": "ed3875247788398142bf50a42def4acd00988688e272a94a4dddffc8a2c9bf87",
    "V2LaneWorkAdapter::push_effect": "8974bca860609c853efe07e78397cb4be80e8bf1a688831bf1c28b1807293441",
    "V2LaneWorkAdapter::schedule_retransmission": "7468d25a90d61258242527880622e74ff38143c0f75c2e7bf572c9792c9f6232",
    "V2LaneWorkAdapter::schedule_retransmission_at": "0bda995ee82b67dd357ce7eb6556b5e80c94db96e15cfb40d89059b42539b849",
    "V2LaneWorkAdapter::prune_finalized_merge_sidecars": "5b811e359c0b8a0e7db463be92fc13409b99405e45dd20c57719442abcac7052",
    "V2LaneWorkAdapter::sidecar_effect_slots": "13a99e0350b8d59489cc591573adcc2b1f3f775308e9b225f1672649235699e6",
    "V2LaneWorkAdapter::next_sidecar_effect_selection": "614006f927ec384764396c8742a3b05b5a70907dddd8bcf39ed2db0aba4d8975",
    "V2LaneWorkAdapter::push_merge_sidecar_post": "89e2c9f8586ac17ba40acbcf14b133193ff0aade68ce717ec24c2820dce07301",
    "V2LaneWorkAdapter::push_merge_sidecar_post_or_restart": "85ca2f652d8409223ea2fd331d3c9010710301b5aafc0e2082e2b63557eed2e9",
    "V2LaneWorkAdapter::remove_acknowledged_sidecar_retry_effect": "14002f78eb6eee073c72b1c5fa547c69187392e1a2085d8bd5c1e385fbbf2efb",
    "V2LaneWorkAdapter::acknowledge_certified_merge_sidecar_chunk_admission": "60809702ee6e6fca12c654f9e93a453e9a4098be03608ccb11a6513cc5a6b5c5",
    "V2LaneWorkAdapter::push_merge_sidecar_effect": "08a2b77d14faf80060a2ca76a491df474dbbf548b3f0f4a5f95c13d14102c750",
    "retryable_sidecar_server_control_has_writable_route": "4f7ac1895057d195e13c094178010703e09198a4b6824a96b75739072c1362eb",
}

_PRODUCTION_LANE_EFFECT_PREFLIGHT_EXACT_SOURCE = """
fn preflight_effect_insertion(
    &mut self,
    effect: &V2LaneWorkEffect,
) -> Result<Hash, LaneWorkEffectInsertionOutcome> {
    let predecessor_ready = match effect {
        V2LaneWorkEffect::PostLaneBlock { message, .. } => { self.outbound_lane_message_predecessor_is_ready(message) }
        V2LaneWorkEffect::PostDurableLaneCertificate { certificate, .. } => { self.proposal_predecessor_is_ready_for_progress(&certificate.proposal) }
        _ => true,
    };
    if !predecessor_ready { return Err(LaneWorkEffectInsertionOutcome::Rejected); }
    if !lane_work_effect_reply_routes_have_valid_shape(effect) { return Err(LaneWorkEffectInsertionOutcome::Rejected); }
    let key = lane_work_effect_key(effect);
    if self.effect_keys.contains(&key) {
        return Err(if self.effects.iter_mut()
            .find(|queued| lane_work_effect_key(queued) == key)
            .is_some_and(|queued| merge_lane_work_effect_reply_routes(queued, effect))
        {
            LaneWorkEffectInsertionOutcome::Duplicate
        } else {
            LaneWorkEffectInsertionOutcome::Rejected
        },);
    }
    if !lane_work_effect_reply_routes_are_valid(effect) { return Err(LaneWorkEffectInsertionOutcome::Rejected); }
    if self.effects.len() >= self.limits.effect_capacity.get() { return Err(LaneWorkEffectInsertionOutcome::Rejected); }
    Ok(key)
}
"""

_PRODUCTION_RUNNER_ACK_SEAM_ITEM_SHA256 = {
    "claim_runner_lifecycle_process_generation": "b4bb67413cbfce25355d34b5342d81a96ad7674614e4134b48f7c74cd49af315",
    "preflight_finalized_lane_rollover": (
        "883edeefab8ca2ed10e688c58e591949a044e6098b960d6bcf16f1858e42d6ec"
    ),
    # Approved together with the exact-output alias after the focused
    # non-descent and handoff mutations passed against the reconciled component.
    "rollover_finalized_height_outputs": (
        "57da29721e6df2151d024228810103c3e65c04789a7ac19b06d8a39b705b6104"
    ),
    "require_peeked_lane_work_effect": "bb5763cb4c16586460c17c92f9578a5431c976fb83bc512e94e84646d6e5c1da",
    "lane_work_limits": "6597822785d94c22554d152b4c403b425c7a11aab1a19059fac41368332202b2",
    "apply_bounded_sidecar_admissions": "27eb4ede4dd038babb38255b89f6a25259b79f55c6dcee33779efbc5d91e04ad",
    "apply_certified_merge_sidecar_chunk_admissions": "0243d1f22247947cc44ac474293a9c852c63509fd46f9357e4ce56b3fd0be518",
    "apply_certified_merge_sidecar_closed_prefixes": "4d27f99c125389f9ffd2cb85b752445882043824f7d5427edc00838ce00417f9",
    "apply_certified_merge_sidecar_closed_prefixes_with": "8a4969958909c1f7b00e17c71597c4f731dbd3eb0803dfc1a60af0d5250a158b",
    "retry_exact_output_and_apply_sidecar_admissions": "e2cabd14191a8f3a4266a5936192a553850b93c86899c085bfbac302f1b9621b",
    "apply_retired_historical_recovery_requests": "01305939d7475cf01935b15f2c66bb6a9654089bccee669a850fd5a839fe3cb0",
    "apply_retired_merge_sidecar_requests": "9ca51c80ef5198d6ab06ab076ffa7d4719972cf109c868f25437a3f59acf7539",
    "apply_acknowledged_merge_sidecar_closes": "87b4efe9a37159d6ba9e6c6f9c992f49afcf6d3727d83e10cf54ec6c367e9dd5",
    "dispatch_lane_work_effects": "a26b7238a9a62db73e31134d6b6722ccf22577166cfead1d881f863040d4f139",
    "dispatch_lane_work_effects_with_progress": "3eb4bbb2474d45ee9e51283c5165e5a82ee25c9fabd4b90a6e4d4eeaf9d5d9c5",
    "drain_finalized_lane_work_output": "fe49593396950970f1e2dd67261c2d50c6374db5ce80114b0caebc8cee266f97",
    "retain_active_owned_reply_routes": "bafe4c316b7d50e5b89bb9468dcf47271985b5f17f8277cb7c70bac5df74be87",
    "retain_active_owned_reply_routes_with_snapshot_hook": "4c941bd7f4f914f2fbe919467ac791f7c9fbeb25a663a043cc3ec3b42a263043",
    "dispatch_lane_work_effect": "a49cda3f020e0f2f577bdbd7e1c8e9b17fa529290a9115755c5665713973c278",
    "dispatch_lane_work_effect_from_snapshot": "20b07ac620f07eca9a61e14f198473a5beb9fdf77bf5222d34f0d7338791ec47",
}

_PRODUCTION_LIFECYCLE_EXACT_OUTPUT_ITEM_SHA256 = {
    "ordinary_loop": "fe38b2b2ab597569383e9b693deabb75346eee956713ec26e3c5174eca38f767",
    "pending_loop": "2e085e0ee59b8bc0656a82aaccd48512d3a282b2799289145851677efea52d8d",
    "ordinary_finalize": "05a36cb47c73bd91e88590bfed1eb0f078a5c75915a5f314497fb89f352aa041",
    "ordinary_active": "9318c085a66e2314d77962d06167e91a14327ba395377b01629ed756b65979e6",
    "pending_active": "2c6ba74ed2d7a1893c41305aae5dd50a4722216320f5c403a70c2200ac407fb9",
}
_PRODUCTION_ORDINARY_INGRESS_CONSUMER_ITEM_SHA256 = "32e0bf9fb84c2f4ef83672eb6ee22ee14c0d89052cfcbb391cc10226730bb210"

# `asyncNodeServiceDeadlines` is a proof-only projection of this one explicit
# trusted runtime contract. These complete-item seals bind the structural
# production half of that boundary: one local serialized height loop, finite
# service batches, and finite idle waits. They intentionally do not claim that
# Rust proves host scheduling or I/O latency; those remain the trusted half.
_RUNTIME_AFTER_GST_REQUIREMENT = (
    "After GST, each non-crashing responsive validator participating in the "
    "active-height or exact historical-recovery corridor has an advancing "
    "local monotonic clock, its serialized height runner is invoked within "
    "the declared service bound after every finite wait, and its admitted "
    "local fsync, signature, reconstruction, validation, and application "
    "work terminates within declared service bounds"
)

# The first-release Certified-Serve path is owned entirely by LifecycleLedgerV1,
# its concrete registry, the complete Ready census, and one dedicated worker
# command/completion family.  Complete-item seals complement the semantic
# token/order contract and make removal of any reviewed owner fail closed.
_LIFECYCLE_CERTIFIED_SERVE_ITEM_SHA256 = {
    "registry:ConcreteLifecycleWorkRegistry::attest_ready_certified_serve_request": "310178621cd80f93a0fd32ffa4de5b38ca55237dcd0e5b2d3e09d2e051da4dea",
    "registry:ConcreteLifecycleWorkRegistry::project_claimed_certified_serve_dispatch": "985dcb45690f99cde6bc2b07703e9c2cf915ceb1abb43b99710e8be71c4f5f26",
    "scheduler:CertifiedServeSchedulerObservationV1::from_live_cuts": "886ef927aff3c6f8ae8577bb1bcb729f4058b48f20df6c5fbf1c3773c43b3d4f",
    "scheduler:claim_certified_serve_turn_v1": "93578174a9077b0cda5510e9f763b416731fd5bcae84fcc3c124949c8a3535d1",
    "turn:prepare_and_dispatch_current_certified_serve": "46c5d50370203b3e8cf81f37651da7610edadd11b6368ea03e8d36a46a5062d6",
    "turn:LaunchedProductionLifecycleV1::drive_completion_pre_gate": "aa3fc3f5004c108b142df2fb92abdd47af5cb220bfdab36d2eb5d93a63ac5466",
    "turn:LaunchedProductionLifecycleV1::drive_ready_completion_turn": "e11420f7da18a84d52f0ff8e7f38ac86edba92e98c5a75038a08d2783cfcddec",
    "turn:LaunchedProductionLifecycleV1::drive_ready_completion_turn_with_required_ordinal": "80684a81b81aee02838428aace8d22dc66cc309e5eeaf1697e096c620ea60be8",
    "worker:LifecycleCertifiedServeTaskV1::from_dequeued_parts": "2fd97e5ef2efa718390f2b3aee03c357aae44562682f85288d973a4739e7cc7d",
    "worker:LifecycleIoCapacityReservation<'_>::preflight_lifecycle_certified_serve": "698cf37eefa08f22e3fc49a3bba6e04fce8c4e031e94ba6922cde786ce78556c",
    "worker:LifecycleIoCapacityReservation<'_>::commit_lifecycle_certified_serve": "f1edcfbd0f5c22919a05212cf7de00e5f914d551ca86f6b5b6156dc254e3d3f9",
    "worker:PreparedLifecycleCertifiedServeCompletionV1::settle_deliver_and_acknowledge": "1c91ff556738f1f0c0ba826da163173aaa8bea1d061356e4949c9bbe179d031a",
    "worker:ProductionV2Services::post_to_peer_on_reply_routes": "326e01fb46a4f99e7bd8c3b09f3216252b74ee7448003a2640dec578e4a8c08f",
    "worker:ProductionV2Services::drain_lifecycle_certified_serve_completion": "689949b5b5c84b99ae0c5ef0124b264111c358b9f1f2da88a2700e26ce5a4fbf",
    "body_store:V2BodyStore::read_durable_body_for_certified_serve": "c3e4d12afaa3f18ad1d5b0865eb3a54f4ad892eff4449b9d9d5b72bfd8f26c87",
    "projection:super::ProductionLifecycleOwnerV1::settle_certified_serve_worker_completed": "77930bc25fa0078aba267238ba1f3e8016eaa28f9a2d3aa58b4199c3f3333d10",
    "projection:super::ProductionLifecycleOwnerV1::settle_producer_turn_advanced": "d15b6ada19aa19ddd64ce9a23ca4ec06518cb4eb99cfe386976f7f85bc6f3917",
    "ordinary:run_lifecycle_active_height": "9318c085a66e2314d77962d06167e91a14327ba395377b01629ed756b65979e6",
    "pending:run_pending_active_height": "2c6ba74ed2d7a1893c41305aae5dd50a4722216320f5c403a70c2200ac407fb9",
    "height:drain_lifecycle_v2_ingress": "e2ddb2b2ecef95315509efadf7432b2d0dcb6c57ad5670f615fb7852038b8fde",
    "launch:ProductionLeaderWireIngressBindingV1::bind": "a2c191a1ada7ec3b3dd00c36c4f495b1ed6c06e2527b2ca9e68b3729f8071f81",
    "launch:ProductionLeaderWireIngressBindingV1::retire": "b2aca6532fa807ad78a8cbd4d202152209c53dd5dd8c5a4fd5bba45f7df18c4d",
    "launch:ProductionLifecycleOwnerV1::launch": "d3b9a9f68ce361cb3609c5b181869ae483ca76ac3eea33c0a72ad6f57ba76b48",
}

# Completion provenance remains separately sealed because the direct runtime
# census consumes evidence without transferring worker ownership.
_LEADER_WIRE_PHYSICAL_INGRESS_REGRESSION_TEST_SHA256 = {
    "restored_productive_retry_freezes_the_current_physical_source_prefix": (
        "15e344ffc278063de2c2d05d2c22fc6827b5483ba0d4dede59bd5e13c5bb8d58"
    ),
    "restored_older_logical_owner_cannot_cross_an_earlier_physical_leader_wire": (
        "25864dbc9942b3f019ffe1783d80e93361194be4e25f2490e60fda2efb61762c"
    ),
}

_LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256 = {
    "FairV2IngressCheckedSelectionScope": (
        "55e89301fa99df237f13dcc8f81d8279c4cc2ccf856922698bae7b315bd0c57c"
    ),
    "is_lifecycle_lane_local": (
        "e567836e91e1df241c9d1480ef3c5c29a862fcdc7e1d9a1b856b9a2ec1355f52"
    ),
    "try_recv_lifecycle_lane_local_checked": (
        "a9002be83f47a532e07cb873352ade6d5e5a455ee191fbb8f1a6e7f317d34311"
    ),
    "fair_v2_ingress_admit_leader_wire": (
        "baaa66eb2c3508f15281f706ad1e2529904430781ebc574a420cce723a9c4b4e"
    ),
    "try_recv_if_at_checked": (
        "73722eaedc36f6ef5265f77198fb95ea520b686ea71406cca9326a8376c2c13b"
    ),
    "try_recv_if_at_checked_classified": (
        "ca657eaedc48fdfdf96aeca1558d4b17774762c22fd0cd7e8cce82270aff5487"
    ),
    "fair_v2_ingress_leader_wire_selector_projection": (
        "76784616876c15608e352a93941912c2002d252ceda70ebefedf4a10495a8730"
    ),
    "fair_v2_ingress_queue_gate_verdict": (
        "217a9045fe04898fdbe2190451c990e44f75fb54b07f73197fb88a755a0cf75d"
    ),
    "ingress_scheduler_ordinals": (
        "994beede48b0f3f8b0418f2eac37029ca5f65fc934aa4206e9dfc69d1a2acefe"
    ),
    "bind_leader_wire_lifecycle_gate": (
        "6e28cc9ea4d84a58f77736f20459030f6591ace7c3e33fae673ad2c5bc4f4c25"
    ),
}

# Exact comment/literal-free source seals for the production-shaped selected
# Serve timeout-recovery regression. Each seal was approved independently after
# its corresponding digest-refreshed semantic mutation was rejected.
_PRODUCTION_WORKER_ACK_SEAM_ITEM_SHA256 = {
    "PendingExactFanout::classified_with_route_history": "a1f9ad9102ffddade0c83bac4895a781a72ba7bec980d99e914eb0641dafc9e7",
    "PendingExactFanout::classified_with_reply_routes": "84c22ccbeecfb531f69c7a03b6f659281fb69fde25f013e0aa394afba347914a",
    "PendingExactFanout::retain_active_unowned_reply_targets": "d34c8c2d4f0f4e402bae740f6463445a30453b88761fcb125cc5ac506479819e",
    "PendingExactFanout::reply_target_merge_plan": "d4470015919d5a1d7838b0c33a8a8c2c545808eac2e73e7ed8e5d37db2be0653",
    "PendingExactFanout::reply_target_merge_plan_with_hooks": "7fd055d3df0b09abc838ebcef9d21f9dc98020bcdd162f1ffc4a7f4fda697ef0",
    "PendingExactFanout::preview_coalesce_plan": "f911b55bc95a4ea4ad07902c651cc090ebe5fb2d488a4adabbd86e22e7b720b2",
    "PendingExactFanout::commit_coalesce_plan": "a311d1d98cea528b88438f90a8f1d6e87b5adbd1d1b158730765b4e66239cade",
    "PendingExactFanout::retryable_certified_sidecar_responder_control_target": "6ec5aac36548c23c407f8212aa93e9794a1383f4ce20c517ac6c158f24193cab",
    "DurableExactOutputServiceOwner::is_sealed": "e739a883ce93dc761dcb9ce0673f69e2e280562ab67f3ca94d1a454efa0860b6",
    "DurableExactOutputServiceOwner::seal": "abe0ec8f7915faab4687fa7be6606ab69e8b58f650cf4ba62bf8610eb586e652",
    "DurableExactOutputHandoffReceipt::is_bound_to_transport_owner": "3a991fb6c233282aab23f9e1cf30857457983736bbd15c7da0f25423eb76a2b6",
    "DurableExactOutputHandoffReceipt::matches_predecessor_context": "7e6c407fea5b7393d085f3ae32a99832b5b9cf8906e64c5a168bacdd46ca1dcb",
    "DurableExactOutputHandoffReceipt::matches_finality_artifact": "2e0593d1f00ebd5f7fd0717ad6123cbb4c37a4c27e45b2b514e5dfbb5d37d4cc",
    "DurableExactOutputHandoffReceipt::authorizes_immediate_successor": "0d432476a9154613c9bcb9285a7d69a0dd8264e818dc993a12e77903d8d3e069",
    "PendingExactOutput::new": "896bc8e8ca1465d394a00339826cfc8c8a4ac1ed04f9b17e7e9d06f25fa8398f",
    "PendingExactOutput::is_pending": "6cd7ecb71f163b7b59e59abcce7f413a4eef60bfbd160950c4afd03a0ff73588",
    "PendingExactOutput::remove_fanouts_matching": "d0734534395316bdcbc632c4270a433fb016975e5a096f94b90b4c0f2ffa48c7",
    "PendingExactOutput::close_certified_sidecar_prefix": "e770e06e0b2c4b42a89e1a62bea7f5a34dae4fa8e276ee8bff58d5a80b35532b",
    "PendingExactOutput::cancel_historical_lane_recovery_requests": "1aa47eb7ff008f21e37f0fa87c6df6cc595c69cf4c48e4aee3d60a45b508402b",
    "PendingExactOutput::cancel_certified_merge_sidecar_requests": "4692c2f28ba62b05ecdbdfd0794d41b22fe03e85fdad65cf104e17590c6173b0",
    "PendingExactOutput::cancel_acknowledged_certified_merge_sidecar_closes": "a451b42b519b347c4e42169c29a053111054983f5ffd2b346496960edc63ea82",
    "PendingExactOutput::pending_sidecar_flushes": "14c74fcdfe37c137fe20897ab19acbac746b9d3a4f764a939abbbe1bc43b2048",
    "PendingExactOutput::sidecar_control_units": "e5eeb08ba9065c86d9b13abbf45b991864d538339f09b2dfe42e5afa0b84ef68",
    "PendingExactOutput::restore_pending_flush": "ff20f9c93c8cbab55fdeef391e73223cba5bfbc0bf4fac2b9c8130b984ac0d7e",
    "PendingExactOutput::poll_reply_flushes": "eae8ee4dc4996b077b9d0e3315e96e8c35a18b0189f2add40e898e60a4167749",
    "PendingExactOutput::validate_owned_reply_transfer": "c39a07ac424ad25dc1d2d1d5cffec3daacbbd7a83c029ff289739745acb6f591",
    "PendingExactOutput::can_enqueue_owned_reply_transfer": "8fcf9c1dc5edb24b0001104718bf80ff142716e45e36b179f74a430e2b214c38",
    "PendingExactOutput::enqueue": "b6379336a656f578037f65bb7b297529092f3f64c06bf38ef5f590a4a3aa81c6",
    "PendingExactOutput::enqueue_owned_reply_transfer": "285ad0dcddadc2a95c155a520f636d14932f6c9c2913e7eea7e43cf443e6642b",
    "PendingExactOutput::project_sidecar_receipt_completions": "e506155b90096fb5980cb878611a5c42190591efebcd7d10d3961061cf925158",
    "PendingExactOutput::retains_retryable_sidecar_responder_control_for": "8f2960dccb011be19e3cb0135ba26138f4020dbc91333bbcd1a73cdcae8d17c1",
    "PendingExactOutput::enqueue_validated": "ffacd5f5fb3424e183c23eb05f021e715d8c999d3a519f165badcf5c206c11e2",
    "PendingExactOutput::handoff_applied_height_to_durable_reconstruction": "e78d702c927524d363d59d1a098bfd6649d6d399e3f0252faac9d500c77b5a80",
    "PendingExactOutput::drive_with_budget_ack_and_durable_history": "334da253eab1b11913ae2f909d162c31f8e2022c4f11fa3aec141b3644a44d52",
    "PendingExactOutput::drive_bounded_with_ack": "3576ef1066ef4460f076423c8f360b0f7e232fb75d281965b4e94ccc68f37726",
    "PendingExactOutput::park_unwritable_reply_target": "bae33ea7bcf13a905da400e913bed5bf2347f103d52be229e0d57fc6f24376d2",
    "ProductionV2Services::admit_network_exact_output": "8652c4a435eeba2522055d198d1bc997befa05615097688986be0d4cb0d1f460",
    "ProductionV2Services::drive_pending_exact_output": "2e2cfd4ff54577ecd72c948271cc492865c1ad00bb7e9675885d2d151261f010",
    "ProductionV2Services::enqueue_owned_exact_reply_routes_while_guarded": "e26271d8dee4d4a3edc25b6619ca182965b3f90b9b38f945aed1c4b0c632a3ff",
    "ProductionV2Services::retry_pending_exact_output": "0f86ca2f246cd92b34ea9b7950815b2f18fdf0e9aa2e25379703ca51e97b72d6",
    "ProductionV2Services::has_pending_exact_output": "107becdd3b504739250f12867eecdb459362f907957797ea3a0e31a71e360768",
    "ProductionV2Services::drain_certified_merge_sidecar_chunk_admissions": "d065d7b2b625852ed7ecd8458997c763b8c6d4d1ef445275150c16a62f1badf6",
    "ProductionV2Services::close_certified_merge_sidecar_prefix": "fb6879c8c325aedc7c19f093a9e5b2cb1b8c25c7bd89c7da1f2694e923cde3a2",
    "ProductionV2Services::cancel_historical_lane_recovery_requests": "e8e7c5bc0c0028e33ad1958785c1fb95bdb1d14ab15b3285447107b537f3f27a",
    "ProductionV2Services::cancel_certified_merge_sidecar_requests": "d18166631ba1b3a76813bb1b403e817980a7e54319d2bdee527d088f79bf6447",
    "ProductionV2Services::cancel_acknowledged_certified_merge_sidecar_closes": "76db166a4c89806aea84964d4c16953783b98188fb73a89304375b78fbd095a7",
    "ProductionV2Services::can_retain_lane_work_effect": "98ac80aed994f8d87e0a9841d14064b4434d9bf4e686ff3ee6cd57e917190b6f",
    "ProductionV2Services::can_retain_lane_work_effect_from_snapshot": "8bbf87be5675b7014d08b93630e4426ba77de26915cbfb20cb6cf817e26943ef",
    "QueuePlanBatchSources::resolve": "b6173b2cee5ef3b71c9ef224ab9f47ee79d91d2261564d0f5bc0b096b9004988", "QueuePlanBatchSources::contains_exact": "51b5e347cb96511f4bc0ea73489b59cf5aef73379b234025a069f9256de50f57",
    "QueuePlanBatchSources::validate": "3ca69f4bd49e9a9512e2f423b1ff60683d6b4c4b4f7450a56bd47c0f6dc73dbf", "ProductionV2Services::post_queue_plan_admission_certificate": "c8e65ca9bd8d37e1f79c0300208c6e9ed06dce3c964dd4254a9faa0110179581",
    "ProductionV2Services::queue_plan_admission_batch_sources": "118c898268b5ad6d8df8c53fe876857a91061c50eefc8ba11a019cb8d474f1ae",
    "ProductionV2Services::queue_plan_effect_parts": "870f2bf465ddd3dbd6f828c9a15a29d8a1d3a44e030f4ede518c09ce952fc00a",
    "ProductionV2Services::handoff_applied_height_output_to_durable_reconstruction": "7444ffcb40c52af925865f03520de3eac64f59d1c31ae6e549c5bf92e17aaf84",
    "ProductionV2Services::seal_applied_height_output_handoff": "293b5cf2fc90356761639f3be19586dc6e0ded95b1f6ab69a1c34dd89fea77ac",
    "ProductionV2Services::validate_applied_height_output_handoff_authority": "7e122f2997f6fa75a67033addc960c72e2dad0dd766588c2d667e056b9363bc3",
    "ProductionV2Services::finish_height": "6076b85d10499e42453b033c749db674c1f96582f7e901730533fa04f649a564",
}

# Exact token seals for the frozen validator ownership-unit geometry and
# independent authenticated-source attempts. Qualified keys are used where
# common method names occur in several worker impls. The semantic checks below
# additionally bind atomic per-source route merging, non-regressing cursors,
# roster x class reserves, and shared-unit accounting.
_PRODUCTION_EXACT_OUTPUT_RESERVATION_ITEM_SHA256 = {
    "PendingExactTarget::apply_reply_route_update": (
        "e75cca425a7afcbd98a0eedf225f6586dd333a06afbb38366e0660b591a84fee"
    ),
    "PendingExactFanout::classified_with_routes": (
        "c848ccf3f2e54014325dc8b27794a5f9771a7df45f6ed9aa16ccc03451145d18"
    ),
    "PendingExactFanout::target_source_at": (
        "1214b0b8088d93df32646db104442dd46d56dafd53b4cceb14afedff54629cd9"
    ),
    "PendingExactFanout::outstanding_sources": (
        "0acccc51a9bedd5fec75171eada06cfa27036daa8b29d74dfebe9ee8a65f0078"
    ),
    "PendingExactFanout::outstanding_sources_excluding": (
        "5ed556d54c561a57c50d7636f84fdefaf29e14d1179de7edc245447ca861f1be"
    ),
    "PendingExactFanout::outstanding_reservation_counts": (
        "02972f0bfa1e31b0dca617382be3254a8da07177396c69101e068cb02075b3fc"
    ),
    "PendingExactFanout::target_reservation": (
        "77bf8f89df068b9aeec3c6d9d8359129f162ca3543460bcbc407fd442f3c4eb8"
    ),
    "PendingExactFanout::certified_sidecar_topology_progress_target": (
        "fcb909658b3a0e546a8e6e5379ca4437ce4e55c2262f88e7ebb59ab7d8ff428b"
    ),
    "PendingExactFanout::admission_reservation_counts": (
        "3b5e7800f133b7673650fee75acefd31e56316c2d33ea5405c2b898fd11a8659"
    ),
    "PendingExactFanout::reply_target_merge_plan": (
        "d4470015919d5a1d7838b0c33a8a8c2c545808eac2e73e7ed8e5d37db2be0653"
    ),
    "PendingExactFanout::reply_target_merge_plan_with_hooks": (
        "7fd055d3df0b09abc838ebcef9d21f9dc98020bcdd162f1ffc4a7f4fda697ef0"
    ),
    "PendingExactFanout::coalesce_reservation_additions_for_plan": (
        "081e95192953390b7b1b17794f956c9e089d7986bda1c55ce49ddd83ad4f0979"
    ),
    "PendingExactFanout::preview_coalesce_plan": "f911b55bc95a4ea4ad07902c651cc090ebe5fb2d488a4adabbd86e22e7b720b2",
    "PendingExactFanout::commit_coalesce_plan": "a311d1d98cea528b88438f90a8f1d6e87b5adbd1d1b158730765b4e66239cade",
    "PendingExactFanout::can_coalesce_retry": (
        "b5f48359f0342b142f04b6fee8c6e74e7cbeaf5068f1dc628fffe7ad5a971de5"
    ),
    "PendingExactFanout::has_writable_reply_target": (
        "a717b59e74c10dda4248c99527c781463eabb0c8ac355f2e718f5b3f3e0f0b9a"
    ),
    "PendingExactFanout::is_stranded_retryable_certified_sidecar_responder_control": (
        "ce27592a9a7e240a207c1b0ea26340edb7e7f25d11f15bdb7ef3d0698446ef49"
    ),
    "PendingExactOutput::new": (
        "896bc8e8ca1465d394a00339826cfc8c8a4ac1ed04f9b17e7e9d06f25fa8398f"
    ),
    "PendingExactOutput::ownership_addition_load": (
        "c566f3dc97560d01457335a290f876f96f5128236bf8cdbbeda6c0c6d14e50ef"
    ),
    "PendingExactOutput::ownership_capacity_available": (
        "078ce17f80c831c8018269a75c2955852a9088e4a150eac969bbab05fc0e2119"
    ),
    "PendingExactOutput::source_fifo_owners_after_fanout_replacement": (
        "2826119bad851abb20d3e7f838ba7df428b9015ec26a448ab0c5536fd3c0fc3a"
    ),
    "PendingExactOutput::ownership_state_after_additions": (
        "a22e2ff976a86ec6218c8a417b2f9b3fd066d16bb9b8edd561fcc00512008ad2"
    ),
    "PendingExactOutput::ownership_state_after_replacement": (
        "80d59ecc4e9300743cf22d82a07712f9c585cbda0cbaf49197ff89bfefc66932"
    ),
    "PendingExactOutput::coalesced_target_geometry_available": (
        "16637dff951694ae9999357a0121a18e520dda07935fb88839625d080e47f403"
    ),
    "PendingExactOutput::remove_ownership_units": (
        "028bce9d7aa7c84ad347212ecfdb1b76066ac80fa9e713219a89e590ef0715bc"
    ),
    "PendingExactOutput::validate_fanout_bounds": (
        "1b40b3085feb03a957e28a2bb7776ce874cd571f0206148aa00126a7dd57bea6"
    ),
    "PendingExactOutput::capacity_available_for": (
        "214b477939e4f6cee1abebaa5bb15b9f9ed03aa211ad3e4d2844790c811982b6"
    ),
    "PendingExactOutput::can_enqueue": "c708eddc4e95b6afc568b0f492a759cb3f7bd0828e988ce75a4d449ea33fdae4",
    "PendingExactOutput::stranded_responder_control_replacement_index": (
        "5a93e8d4bcf188928987ad45e193a7569f25c68f05ca9d7f25a93ba3e96974df"
    ),
    "PendingExactOutput::responder_control_replacement_ownership": (
        "3b28dfe491d895c31600f4c6f871fb04015f7a9214dbb6768431c8484bf6dac6"
    ),
    "PendingExactOutput::responder_control_replacement_available": (
        "1680843d65a96ef27f9c5d68c968dbbea10998b7641194647adede693c3fb4c7"
    ),
    "PendingExactOutput::responder_control_replacement_plan": (
        "1a914ee0f02c0e45e7297282a6a9d415761d610ef303665bd98951ef6136fb79"
    ),
    "PendingExactOutput::replace_stranded_responder_control": (
        "5b87149fac9b75e1690787b5167ed04681676a186f0bd2bfb834ecc5a3c748ff"
    ),
    "PendingExactOutput::enqueue": (
        "b6379336a656f578037f65bb7b297529092f3f64c06bf38ef5f590a4a3aa81c6"
    ),
    "ProductionV2Services::start": (
        "80da0d69fc6dcdb2d8c4b0eddf28b0b7069d56e9125981889d3d99c7174a961a"
    ),
    "ProductionV2Services::start_inner": "9cb6278ea725b3889ea622bfe6627e30eb0dfecdf071b8d1d7338e96e8a97ea5",
    "ProductionV2Services::exact_target_geometry": (
        "978520459f9dd3c5459478e222418ffed2924445c40a79722c307f97e6d28871"
    ),
    "ProductionV2Services::can_retain_lane_work_effect": "98ac80aed994f8d87e0a9841d14064b4434d9bf4e686ff3ee6cd57e917190b6f",
    "ProductionV2Services::can_retain_lane_work_effect_from_snapshot": "8bbf87be5675b7014d08b93630e4426ba77de26915cbfb20cb6cf817e26943ef",
}
_PRODUCTION_DURABLE_HISTORY_WORKER_ITEM_SHA256 = {
    "durable_history_source_covers": "b49d9bd57e7014963961f563d7e31eb5ebead7d9382a949124ad119602c4adde",
    "queue_plan_admission_reconstruction_covers": "1cd167989e8fca4abb7cf25e9bf470b7fa660a596c7afc3eeb0c819cb2c730e3",
}
# Typed semantic claims are the production authority which lets exact output
# cross the applied-height boundary.  Seal both the claim validators and every
# production constructor so an untyped producer cannot hide behind the
# scheduler/retirement digests above.
_PRODUCTION_EXACT_OUTPUT_CLAIM_ITEM_SHA256 = {
    "accepts_superseded_reply_delivery": "b96c62bc732dbe826dcbdd990524bcb43c25e197162e126efeeab235179f476f",
    "covers": "48529c793eedab83283aa2d471486e8bbd77ce399c0be52576bc23ee3b8b540c",
    "from_request": (
        "790314790a852bac49e79b6c71bbe07931ade269d273c536978b45f4822a4b68"
    ),
    "from_chunk": (
        "73871d46bfc5ff28b4db03bef64811ee26e995795dca98c552f5dc8b1e4067f9"
    ),
    "native_amx_message_body": (
        "90737e4116833fb086b7c1f3a7a04dbb34fc9157385103f3dc1be6e74c127eae"
    ),
    "scope": "205283e66724f43148cd8c8d7aa0275ccd3cd8b5160d31f46ea836b5364c9ee4",
    "validate_fanout": (
        "bf75f7441ed0d4a4bd205ac9347a7882e61c37b0b4355d8c20cc70d7a8b8654c"
    ),
    "validate_non_retireable_lane_transport_fanout": (
        "6f57bc6fc5d7b127d67745f5d042e780aac18e9abce9f4d53c90a1c4ebe934d0"
    ),
    "claimed": "75dcecc8adae80ad5980fb812e4d13af3e7f432605c685985a6c2d0e27a67a10",
    "claimed_with_routes": (
        "b4e6d00e981ae28935489541d49c181941a4a38c1384a823ffc7d8ace27e9418"
    ),
    "enqueue_exact_fanout_while_guarded": (
        "a76f7c05dfbafb57b0b58f4d714936eda47d16a17dbbc7be5d0db88ee399de9f"
    ),
    "enqueue_exact_fanout_while_guarded_collecting_released_adverts": (
        "a4175b7fa12a3b3725baaff7f93af2865208fd47976ae3a6476f78b7631e3ddc"
    ),
    "enqueue_owned_exact_reply_routes_while_guarded": (
        "e26271d8dee4d4a3edc25b6619ca182965b3f90b9b38f945aed1c4b0c632a3ff"
    ),
    "drive_pending_exact_output": (
        "2e2cfd4ff54577ecd72c948271cc492865c1ad00bb7e9675885d2d151261f010"
    ),
    "exact_output_scope": (
        "2c322931cf99b7f7e6484c11c48b4bb570b48bc6a95f240e09fab88eea599be0"
    ),
    "post_to_peer_on_reply_routes": (
        "326e01fb46a4f99e7bd8c3b09f3216252b74ee7448003a2640dec578e4a8c08f"
    ),
    "post_durable_history_response_on_reply_routes_with_permit": (
        "c2f9ac9b07a52814d6ba43a0701ecd6a04cb6ca7cd890efb100330b958c0d651"
    ),
    "post_durable_history_response_with_routes": (
        "c6b6ad7759ad801016087dcd0d172c40685e6ee382d3de51efc5e0557b93f638"
    ),
    "post_lane_block": (
        "1a4be6518deb1d756893951563f23f8017bd840779366054ce9b5a3329531389"
    ),
    "post_durable_lane_certificate_on_reply_routes": (
        "84b9fa1c26b45f636ad7fd2b605b3cb4398a74abb3f515ea9e05034ca7a9e429"
    ),
    "post_durable_lane_certificate_with_routes": (
        "d4463b2b414c7e4c4febf871d63ccabca15b060f3345b820b6f7da9c2f3deb79"
    ),
    "post_certified_merge_sidecar_with_reply_routes": (
        "333334838aaf7761315f2eabb1f34e6cead127ddfd4f1e6282ae116e5c9846bc"
    ),
    "post_native_amx_with_reply_routes": (
        "17d66596b19902ce6ac3da41c7c05a000b42a54a87320f92cb588500f151c396"
    ),
    "broadcast_merge_to_voters": (
        "99b0c80af2876f9b92cd9789605a7040d3e6dec0b4ab11f47edf292aeadf5f59"
    ),
    "post_block_message_while_guarded": (
        "5556a35937aa384b399482ceb30f45cfdbe212c59a10bf10aa9fc6f46e17f670"
    ),
    "post_block_message_on_reply_routes_while_guarded": (
        "c28e740e75f29dc743dc81eda0b34b70e46040ca6a44355b8092cb15a98ba9b2"
    ),
    "broadcast_preencoded_to_voters_while_guarded": (
        "1c22c254ba300a86887affe725dd01e126563ac584f791e9da329fe67296bf4f"
    ),
    "broadcast_to_voters_while_guarded": (
        "bbc43f18d7ee7cc95ecfabc70894ae1b82780edab21ebecb8a60445e68113a4d"
    ),
}

_EXACT_OUTPUT_ROLLOVER_CLAIM_IMPL_ITEMS = frozenset(
    (
        "accepts_superseded_reply_delivery", "scope",
        "validate_fanout",
        "validate_non_retireable_lane_transport_fanout",
    )
)

# The lane authority is the only production witness allowed to supersede or
# reconstruct lane-local output at global-height retirement.  Bind its exact
# Kura/application source commitment and its winning/non-winning validators.
_PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256 = {
    "durable_historical_lane_output_source_hash": "d6f14857c39045bdb388095d00ad11d99be5af7aa3cd84c789569eab24148e8b",
    "durable_historical_lane_verification_pops": "090336f96d80a1ea90e51a2ea2cf4161aca46057c6e11c87cefeeb24c18b36ee",
    "covered_source_hash": (
        "3ac2d2397d5fe256dc01b92458007f9e63c25af50b0cff7be83c0e9e830782e8"
    ),
    "persistent": (
        "3e865b8a1b9b7136ab2cf81dd6d33fe84ae9e9b14eab2006156ae9cbc29520ff"
    ),
    "lane_output_identity": (
        "bf17d20ee94a5023ce4623b31eb333e8d7cb12c1a155d058a2a9f22840430b58"
    ),
    "validate_winning_lane_output": (
        "df5aef9263e9ef898bc5481e57ecf00c80074a3413a31eb55f3638e77d733900"
    ),
    "validate_winning_lane_qc": (
        "345e7f2f2faadd6dc57195390ce8e711549608ba56f1cb01f962250414f4c011"
    ),
    "validate_superseded_lane_output": (
        "ec480ca71d859f5b27fcc31731a1bc2ebb02ee8d5002475d53d3ed14109c94c2"
    ),
    "durable_lane_rollover_authority": (
        "43c30d2794ea46ae16e0c9562b6281c32884f0f69e2aaea57950990e837d4396"
    ),
    "serve_durable_lane_certificate": (
        "bcab2428b4a2d43dd23989bebe917077e84b069c4e120808f6b25ce4503ce52f"
    ),
    "reconstruct_durable_lane_certificate": (
        "d0f552b3786458048dfb987c161bc37f3a55e92680a92d1818a11b658f64396b"
    ),
    "reply_routes_are_live_for_peer": (
        "cdca18bef9df99c77e3698622c9cf6941dd249967bb587f60e9fd381a4f8b235"
    ),
    "lane_work_effect_reply_routes_have_valid_shape": (
        "5b3f71820b5839760cf75c74eb47b0939d9cfab114eee8edd4fe1c0364beceb9"
    ),
    "lane_work_effect_reply_routes_are_valid": (
        "ea6f0f3cf2b270bc7ae3af1d96d90b9caba94fda993f78e618ed520e292ad66e"
    ),
    "merge_optional_reply_routes": (
        "39e76e7cfe0d234d3508852537e442bb6511a51f372cbfded1bf6bd0ff853f2d"
    ),
    "optional_reply_routes_retain_candidate": (
        "5c43c5b723d77fdd98b194a0e86ab37c25c3a165c4aa96c032654238863bca17"
    ),
    "merge_lane_work_effect_reply_routes": (
        "0d32e263ed0d2659f4274b03a1f8bb87f8de386beb9afb8103eaf3342c6b4459"
    ),
    "merge_lane_work_effect_reply_routes_after_route_merge": (
        "13fb5c8e43ada7c5fa4ba3cb4c5dc91cb669638cf8bf24b1b48a1fa5023c636d"
    ),
    "lane_work_effect_key": (
        "78c0efead7f4ec363bdc646d9cb512518fb469e882aec65829543ba517f1aaf1"
    ),
}
_PRODUCTION_EXACT_OUTPUT_RUNNER_ITEM_SHA256 = {
    "authorize_decided_lane_recovery_drain": "d8560f22aa7e0ad56cab370f00531d3338f22bafbc564e73ae8e727086d4f556",
    "select_blocked_ordinary_lane_local_ingress": "cf0b52b8280f229dae97589635f302efc2699477e95fc8655514193d193af94f",
    "drain_blocked_ordinary_lane_local_ingress": "1de13949d673c50dd194f206714564252fb428e43a12cf68eb8f50d24304928f",
    "preflight_finalized_lane_rollover": (
        "883edeefab8ca2ed10e688c58e591949a044e6098b960d6bcf16f1858e42d6ec"
    ),
    # Bound after the exact-output handoff mutation survived refreshing this
    # helper's own token digest.
    "rollover_finalized_height_outputs": (
        "57da29721e6df2151d024228810103c3e65c04789a7ac19b06d8a39b705b6104"
    ),
    "dispatch_lane_work_effects": "a26b7238a9a62db73e31134d6b6722ccf22577166cfead1d881f863040d4f139",
    "dispatch_lane_work_effects_with_progress": "3eb4bbb2474d45ee9e51283c5165e5a82ee25c9fabd4b90a6e4d4eeaf9d5d9c5",
    "drain_finalized_lane_work_output": "fe49593396950970f1e2dd67261c2d50c6374db5ce80114b0caebc8cee266f97",
    "dispatch_lane_work_effect": "a49cda3f020e0f2f577bdbd7e1c8e9b17fa529290a9115755c5665713973c278",
    "dispatch_lane_work_effect_from_snapshot": "20b07ac620f07eca9a61e14f198473a5beb9fdf77bf5222d34f0d7338791ec47",
}

_PRODUCTION_BLOCKED_ORDINARY_LANE_LOCAL_HEIGHT_ITEM_SHA256 = {
    "LifecycleBlockedOrdinaryLaneLocalIngressPermitV1": (
        "8fcb9016d28cc3b21d599d115f45bed1af0097ddde5b114b8317964f95f03d90"
    ),
    "blocked_ordinary_lane_local_ingress_permit": (
        "282616f61a21ea25d33711d6cc35e48e009243dbfd6e6c5a8bf12aeacec081bf"
    ),
}

_PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256 = {
    "height::LifecycleApplyTerminalReadyBroadcastPermitV1": (
        "3eebd86775aaeac2ad4a4b82a0f86f12ca62e9dc443d361886efa7c020dbbd3e"
    ),
    "height::apply_terminal_ready_broadcast_permit": (
        "bf62c9bf3986e68d4cc6b0151a82ca02ac2a40e05050926c879d9b33e8eb5d8d"
    ),
    "height::completion_selection_retries_before_runtime": (
        "dec087d80453ae0b26c089c227a5a0d9aa900ff813ed43eee303937ac506843a"
    ),
    "driver::classify_apply_terminal_ready_work": (
        "44735b2ca2fafa13b5e80b0e909f733dfc96217c0fe101e802e21b9f06befa93"
    ),
    "driver::LaunchedProductionLifecycleV1::drive_apply_terminal_ready_broadcast_turn": (
        "f29600918001515fbeb98d0361ea268712e2c2d6da5bb09d00aa9519603feac7"
    ),
    "driver::ActivatedProductionLifecycleV1::drive_apply_terminal_ready_broadcast_turn": (
        "e799a2ad631f9fc19dcc398f818895694349c7c3f3db4190fdd67832eb1d34a6"
    ),
    "scheduler::ProductionLifecycleOwnerV1::prepare_apply_terminal_direct_broadcast": (
        "1763107673fecdfaff74edb1ab0bf7487093ef8181a91455546e18b61471a99d"
    ),
    "scheduler::ProductionLifecycleOwnerV1::wake_apply_terminal_direct_broadcast_if_fenced": (
        "3ab6946b9d5e91dee03fa1ec6e4aaa0784539609d8508eef477c0d16af3f7b39"
    ),
    "registry::PreparedApplyTerminalDirectBroadcastV1": (
        "3c8b749af4c1acfdc5c515ba25ddb0bd9e6d90508d4ec92f21e227c541d1c56d"
    ),
    "registry::ConcreteLifecycleWorkRegistry::prepare_apply_terminal_direct_broadcast": (
        "9b424cfcdfdd440ccfea0211adcc3677d12098dc2d04657a2bdd3877d8084237"
    ),
    "registry::ConcreteLifecycleWorkRegistry::apply_terminal_direct_broadcast_pending_is_exact": (
        "b8655e67ff593a5c73b1a0264e341ff4ec58dea11fa63ebbb7a6d8f6dd829f34"
    ),
    "admission::ProductionLifecycleOwnerV1::settle_apply_terminal_direct_broadcast": (
        "3225baa482525897247541b84e14644ef34b81984195a1976c28c58561c38b1c"
    ),
    "effects::V2EffectExecutor::settle_apply_terminal_direct_broadcast": (
        "0e65eb068829c5257f158a5b2941183b4709e8c88130f63ac9bda0e84ffc9b32"
    ),
}

# A reducer slice which makes the exact ProposalIntent Sign Ready must return
# to Completion before Producer can admit a later timeout/new-view barrier.
# Seal the authenticated census predicate, eligible-only permit, live height
# route, ordinary-head-preserving Completion pre-gate, tracked physical-owner
# publisher, concrete driver fixture, and its named release regression.
_PRODUCTION_READY_PROPOSAL_SIGN_PREEMPTION_ITEM_SHA256 = {
    "scheduler::ProductionLifecycleOwnerV1::ready_proposal_sign_preempts_bounded_producer_point": (
        "98286d3d592024081c92afae2353f604ecbebf804c749c13e0c9daa26b17c016"
    ),
    "height::LifecycleReadyProposalSignPreemptionPermitV1": (
        "6a1f9f015e100d2c21a2059b2b5ed299c58d4084375d80117f2ff2d9baf565e6"
    ),
    "height::LifecycleProducerClaimDispositionV1::ready_proposal_sign_preemption_permit": (
        "9700af71a07b9b6e8c935f44e6e447c3d2087f89508733c6f124a3d4beedce51"
    ),
    "height::drain_lifecycle_v2_ingress": (
        "bbd77022da85d8d4ae7a7b1114483f3d3437e8fdbce14de7cb702b5716f26ddd"
    ),
    "height_test::only_an_eligible_claim_can_preempt_an_ordinary_head_for_ready_proposal_sign": (
        "dd96ca9fb8271e423099f6a019259cbfa524d73d86f07d1afdd377aa80dc8e76"
    ),
    "driver::LaunchedProductionLifecycleV1::drive_completion_pre_gate_with_ready_proposal_sign_preemption": (
        "0fabc0723a3288b463bf55b2cc7a02638cb1627967ba717b169520b7bee3eaf2"
    ),
    "driver::LaunchedProductionLifecycleV1::drive_completion_pre_gate_inner": (
        "f10f0a6f3b6824d8f264dc9bf30538ad1563d8121c75c14d26d37bbea30c5cb4"
    ),
    "driver::ActivatedProductionLifecycleV1::drive_completion_pre_gate_with_ready_proposal_sign_preemption": (
        "bb485fa1d93cd1748cd6d8f0c7152c4afb78b7bdcbdb5423aa22eb2c55129b77"
    ),
    "worker_test::LifecyclePlannerIoFixture::publish_auxiliary_completion_fixture": (
        "c3b5921a9f581e7ad7bbb44e93ad42de8ec1fa6eb8bd62aaa49cdbefa70327c4"
    ),
    "launch_test::LaunchedProductionLifecycleV1::install_ordinary_completion_head_for_ready_sign_test": (
        "f9f33cf99e1c38a2ebaea00d376fe5bfff7d434620dc46ce260665b7e45c2f8f"
    ),
    "launch_test::LaunchedProductionLifecycleV1::ordinary_completion_head_retained_for_ready_sign_test": (
        "a5bba2319108935316c46e2402dcfce41c9e2e0ba87107dda239344b7cefe028"
    ),
    "launch_test::LaunchedProductionLifecycleV1::drain_ordinary_completion_head_for_ready_sign_test": (
        "edeec434ad30fb28ddc4cb526096bafa6d6f8bd8e8df92995e5350c0f95111db"
    ),
    "dispatch_test::local_proposal_intent_live_wal_sign_fixture": (
        "14ec208611139775c959d7cc44d718925d179c7d822c1cb92bea3018f6215489"
    ),
    "wal_test::ready_proposal_sign_boundary_predicate_authenticates_exact_control_carrier": (
        "c4b40eb74bfcafcbe413991d85044ad6c857d1faf1a5294944705449a13f269b"
    ),
    "wal_test::ready_local_proposal_sign_and_exact_output_precede_pending_timeout_certificate": (
        "97b485f11895e0f3e0273d978498d16caa02850e8bf4d9fee8e7434069865b13"
    ),
}

# Canonical post-dequeue ownership moved into one first-release module shared
# by the outer batch cursor and lifecycle turn driver. Bind the complete tail,
# rather than retaining stale duplicate route/terminal assertions in callers.
_PRODUCTION_EXACT_OUTPUT_ORDINARY_INGRESS_ITEM_SHA256 = {
    "prepare_current_certified_serve_pre_admission": (
        "31022665faf68e0fd519f0e7a27d7b09558ad16ad7b1d2ea073549c162e5ac21"
    ),
    "consume_prepared_dequeued_v2_ingress": (
        "32e0bf9fb84c2f4ef83672eb6ee22ee14c0d89052cfcbb391cc10226730bb210"
    ),
}

# Exact production retirement of the leader-wire lifecycle gate. The retired
# Certified-Serve gate and joint height-ingress wrappers are not production
# seams; the lifecycle contract binds canonical close and atomic leader-wire
# retirement.
_PRODUCTION_HEIGHT_INGRESS_BINDING_ITEM_SHA256 = {
    "runner::close_ingress_for_rollover": (
        "61ae9f7cd71bc2576f9330c5874d2018b873d6144514e9f733f5774343ddd1a5"
    ),
    "ingress::retire_leader_wire_lifecycle_gate": (
        "8d8abbd98ee938fa735d5605710e1bcce16c3db19d401b1a4bec56de9ac49587"
    ),
    "leader_wire_store::park_sealed_ingress": (
        "5941150174b03f6321979beb00b9771125e49f87c3c770d31c6daaef09705a0b"
    ),
    "ingress::close": (
        "24741c2de73120ea5e1a9564203f5d5e0bd9d80a7f1a89d0bca2511e73dee1a7"
    ),
}

# Exact source seals for the configuration-to-worker capacity corridor.  The
# constants are checked as exact source tokens below; these item digests bind
# the checked arithmetic kernels, the user-root call site, and production's
# narrow refinement wrapper to the same geometry.
_PRODUCTION_EXACT_OUTPUT_GEOMETRY_ITEM_SHA256 = {
    "actual::sumeragi_v2_exact_output_shared_ownership_capacity": (
        "e71d1a2376fc34aac057abebde05a41c5b32bbcbad17a409c954845bc4aa64ef"
    ),
    "actual::validate_sumeragi_v2_exact_output_geometry": (
        "b9ad00e3d2ee76b202fa98f53cab9f7264c63a7d9a3050b0c3d57c0449cfb8f5"
    ),
    "user::Root::parse": (
        "8253be717b3dd2da5e2341e3be38c4315f0ea696d65cbb22d785eed9cfc44be0"
    ),
    "worker::validate_shared_ownership_geometry": (
        "67026793b1424da887ccec0301157480e43b3585d298ab1545fe98e8cb577411"
    ),
}

# Exact token seals for the serialized runtime's disjoint C/P/K/+1 queue
# geometry.  The retained certificate is classified from the immutable queued
# command, so arrival order cannot consume an ordinary Completion or Progress
# reservation and several distinct certificates still share one physical
# credit.
_PRODUCTION_RUNTIME_CERTIFIED_FENCE_CAPACITY_ITEM_SHA256 = {
    "RuntimeQueueConfig::validate": (
        "f2bfef98a26b8817ca45fcc55c5669d215ae0768f8f09425baea4bec1024b52f"
    ),
    "RuntimeQueueConfig::normal_limit": (
        "fb3d747ef54e8a0432a6ff94407135ef68f16fc94773609f3975a93ab043dfa4"
    ),
    "RuntimeQueueConfig::progress_limit": (
        "5e87c00d381779d2e6a9af03a4497f1fc6f5eba07cf802de46d14f24e78bf72c"
    ),
    "RuntimeQueueConfig::ordinary_total_limit": (
        "1180f68279ceb396f9d3bf49d2bc3c79e138bb1d7ce2f64aba17f0f75562c27f"
    ),
    "BoundedIngress::certified_fence_escape_credit": (
        "94d47c1be045f5eb4f4b7840550b242df8471480033cdfc839d2614467c96ff0"
    ),
    "BoundedIngress::check_capacity_change_inner": (
        "b32cae42a3a389680b1d3566c21012b9db3aa19729dcb83d0c6a0e127a10cc0c"
    ),
    "BoundedIngress::remaining_capacity": (
        "533205971ebce432b614faaa4fdcdb8c86285ef7aeef75314eab78c12cd59f93"
    ),
    "wire_payload_is_certified_fence_escape": (
        "01d6853abc2e1b0e5f2a84197ce271fff2c4b71ba7467f64e0d7c85b3da8237a"
    ),
    "test::certified_commit_uses_physical_slot_reserved_from_completions": (
        "fe5d361fd09f716cca60ceecab8f4c3acfb1ec01080d16b9851af71277f15e53"
    ),
    "test::certified_commit_arriving_first_preserves_every_ordinary_reserve": (
        "5c1aa2de8dfce58415d400d2d4db69988d63fc43d57090ca31f96202625acadd"
    ),
    "test::distinct_certificates_share_exactly_one_physical_credit": (
        "b8a97dace4bd30ba1cac54a69d12e89640ce9b3cbc6b9c15368a7d17c6728630"
    ),
    "test::invalid_configuration_is_rejected": (
        "6ac0e66e597904f6a3b682eeed89bcb2c185b6ce2a402618071fc0490a645d49"
    ),
    "test::queue_configuration_excludes_one_certified_credit_from_ordinary_limits": (
        "e7ddac2312bc5e327d444eb3e24b022d332032d5534a46c56b09857fb50dd3ea"
    ),
    "test::prepare_qc_cannot_spend_the_certified_physical_credit": (
        "aa348cc05ab9331f2b738eeae41cbd4dc74946fb759ab8e3ed1c41c347e9b376"
    ),
    "test::retiring_the_sole_certificate_does_not_fake_completion_headroom": (
        "4751f1f10ce81f149d80f1a7e65da319d4f397fb21167f3e403e147b872dbd1f"
    ),
    "test::unpublished_body_replacement_cannot_overbook_the_certified_slot": (
        "7e1cff975df51a63a6f0e8a9dbfe11829c22b782622c3a4517feac9c232aadb6"
    ),
}

# Complete token seals for the process-local fair-ingress carrier and every
# exact-output consumer which must retain it.  These close the route-only gap:
# equal payload bytes cannot substitute a different semantic origin, and
# alternate authenticated sources keep independent non-regressing cursors all
# the way through runner, effect, worker, lane, and sidecar service.
_PRODUCTION_EXACT_OUTPUT_INGRESS_SEAM_ITEM_SHA256 = {
    "ingress::merge_downstream": (
        "9542ecd100449b693ebae4c1dbea39f43d651c57533ece26dccb193eab6f77bf"
    ),
    "ingress::merge_downstream_with_observed_receipt": (
        "c097d227ef0a670c2144dffcc30a6a0b18016b781ba6a51728c21b2ee8bf32e2"
    ),
    "ingress::merge_downstream_with_strict_receipt": (
        "e65283e5924e1dfe938cb69289a35067a01d3835027ba65587a32f515ed90032"
    ),
    "ingress::merge_downstream_with_exact_routes": (
        "23e937626dd546ae52105bd4e17033d8785e19dfd6ecca7c0d21ebd840d80231"
    ),
    "ingress::same_semantic_request": (
        "b30874c2662dc448ae573d60fb9892310817975b35dde2bfe5d389d08ada7730"
    ),
    "ingress::matches_message": (
        "258d127f5dabef91f22b0d7cca66637770fe3bd607331068ec6f09bd6567f43b"
    ),
    "ingress::matches_semantic_origin": (
        "73663bdcb60c98f09d052005730ea1b808861e3f0874c50a494e06a05f74a3d8"
    ),
    "ingress::process_local_projection_hash": (
        "fe5bed690e2b4c2e12a1850075fb18c5c7c871e45d70bfe651681b42773f3ee8"
    ),
    "ingress::matches_reply_routes": (
        "98580b7c9e69cc4c69cc32ec6212459e93fba573a335151fec476659d2772c2f"
    ),
    "ingress::project_retained_reply_routes": (
        "6a95a61e3dd7ce8da838fd99693205618d3ec3979ea88b46599ecd68ebfd2fdb"
    ),
    "ingress::advance_reply_cursors": (
        "2be088efc3ff0460b06c0316f7b64062d864d22d8fce286a1474fd1e31af3c82"
    ),
    "ingress::validate_exact": (
        "44cb18bc022237ff922f9c52fd1e7b597410aae74f05a05749862e8cde059ff7"
    ),
    "effects::accept_payload_chunk_with_ingress_ownership": (
        "70a52f79061ceda8b00d9549638ed9dc7825f2c57191491306bbb23d88a5848f"
    ),
    "effects::classify_payload_chunk_lifecycle": (
        "c47df75ef6c648fce8a9f4f13717bef3f2af8e7dc1ad8885225fa5ab910dca6a"
    ),
    "effects::begin_apply": (
        "01c72efc7c8248bb349fd23102fe0a8180ad9bd6bf0500edbfd5d053317631e8"
    ),
    "effects::matches_apply": "8b746ccd8649e40fcb767676099580fabdf47d1b6089999729e15f34cace88a5",
    "effects::complete_application": (
        "beb5a46dd53315316bd1cdca436c083efee56bee66bc45858ab13d93966daa88"
    ),
    "ingress::leader_wire_token_view": (
        "ef9f78efaa117ac2a787a9791e87d5a199978cf1d5a87d8a9e43b94f12e12b82"
    ),
    "ingress::leader_wire_token_matches_chunk_manifest": (
        "062164107c1c1363e538a4ed827b13510b3b0c2a91505d2b77c097f44dbc7a1b"
    ),
    "ingress::leader_wire_token_matches_body_coordinates": (
        "2bc498a9088c735c18803dde3461d50afd70712ac40f7875457290d9796de385"
    ),
    "ingress::leader_wire_token_matches_exact_body": (
        "645a29b8cff1d68fca203fd037f3c665189ab801c1bbe97eca8b6dfdc04e0b7f"
    ),
    "worker::claimed_with_reply_routes_and_ingress_ownership": (
        "8c95a2604897a3dbf327d36721388bf466784f2203869d1fc6aead594ecd7e44"
    ),
    "worker::route_payload_chunk": (
        "cce18c475e177e0413fc6a3912bfa5eac04a9105f27a430abf2356c5cdfc54d7"
    ),
    "worker::has_exact_reconstructed_completion": (
        "2d494b9f5ea950b8fa1e24c3b6f1a9d5f8917dc166658660300d9e66b9064bfe"
    ),
    "worker::buffer_orphan_payload_chunk_inner": (
        "03c11a6a6438f3a0f7cf47cac8f5310b262fd684a9df7891ff906b7f2e0d7de8"
    ),
    "worker::sweep_buffered_payload_chunk_lifecycles": (
        "04d1c1e43bd4c41a0061d7585580907e6d8e4a1d5a2403c06bda1234b3ee51f7"
    ),
    "worker::replay_buffered_chunks": (
        "1259249ed54174dfcecffbacb6e2f70b3a5fbf31e59bd088f6160d3148648920"
    ),
    "worker::retire_buffered_payload_chunk_tail": (
        "fa77f1610c7d58dd73f05cb353d10a4c2a1db4f28b58777138bcfc2daad475e1"
    ),
    "worker::deliver_payload_chunk": (
        "f4341f1f806d10adab9e14b0c4b13c463fc7c6dae22bd7056863548d0912fb5b"
    ),
    "lane::accept_lane_message_owned": (
        "aa4089bc8f8a3ef145430ffbf6737f7c6ce5db0e8637dccefc39aefb6b866508"
    ),
    "effects::v2_ingress_head_can_drain": (
        "482af132caf85c043275bb805966af7e5e8fae3553843b574bb07563b49a5de7"
    ),
}

# Exact comment/literal-free token digests for recovery-scoped eager CommitQC
# discovery. These are production source-fidelity seals, not machine proofs:
# the promoted starvation-freedom target still requires fresh strict evidence.
_PRODUCTION_RECOVERY_EAGER_BLOCK_SYNC_ITEM_SHA256 = {
    "initial_block_sync_deadline": (
        "b3455d656ecac4787561951ec55cc8cde44e2a2c90a3330ad3fe0c65e96c2185"
    ),
    "retain_eager_block_sync": (
        "b5ed7336da6088907bcd26f1b21d2e0bd91a972e8bce68a6e190f238d4c2b56d"
    ),
}

def _transport_geometry_refresh_resistant_errors(
    paths: dict[str, Path], sources: dict[str, str]
) -> list[str]:
    """Check reviewed P2P and production-Taira semantics without token seals."""

    errors: list[str] = []
    for role, item_name, required, description in (
        (
            "p2p_network",
            "relay_message_wire_payload_len",
            """
let origin_len = peer_id_wire_len_from_raw_key_bytes(RELAY_NODE_PUBLIC_KEY_BYTES, flags)?;
let target_len = relay_target_wire_len(direct.then_some(RELAY_NODE_PUBLIC_KEY_BYTES), flags)?;
let ttl_len = core::mem::size_of::<u8>();
let origin_signature_len = byte_sequence_wire_len(RELAY_ORIGIN_SIGNATURE_BYTES)?;
let field_lens = [
    origin_len,
    target_len,
    ttl_len,
    origin_signature_len,
    payload_len,
];
""",
            "relay geometry must charge the origin signature and every exact wire field",
        ),
        (
            "p2p_peer",
            "parse_next_encrypted_frame",
            """
let _decode_scratch_lease = self
    .source_byte_budget
    .reserve_decode_scratch(size)
    .await
    .ok_or(Error::FrameTooLarge)?;
let frame_retention = self
    .current_frame_retention
    .take()
    .expect("complete encrypted frame must hold its source byte lease");
""",
            "receiver decode scratch must be reserved before taking the source-owned ciphertext lease",
        ),
        (
            "p2p_peer",
            "parse_next_encrypted_frame",
            """
ParsedFrame::Malformed(context) => {
    self.last_malformed_payload = Some(context);
    return Err(Error::MalformedPayloadFrame);
}
""",
            "an authenticated malformed frame must fail atomically before any decoded prefix is delivered",
        ),
        (
            "p2p_peer",
            "connected_from",
            """
let peer = state::ConnectedFrom {
    our_public_address,
    key_pair,
    soranet_transport_key_pair,
    soranet_transport_certificate,
    connection,
    network_id,
""",
            "inbound peer construction must retain separate validated node and transport identities, their cached certificate, and the canonical network identity",
        ),
        (
            "p2p_network",
            "start_tls_listener",
            """
let peer_task = connected_from::<T, E>(
    public_address,
    key_pair,
    soranet_transport_key_pair,
    soranet_transport_certificate,
    Connection::from_split_with_binding(
""",
            "mandatory TLS inbound handoff must carry separate validated node and transport identities plus their cached certificate",
        ),
        (
            "p2p_network",
            "run",
            """
Some(update_validator_dial_control) = receive_control_update(
    &mut self.update_validator_dial_roster_receiver,
) => {
    match update_validator_dial_control {
        ValidatorDialControlUpdate::Roster(roster) => {
            self.set_validator_dial_roster(roster);
        }
        ValidatorDialControlUpdate::Topology(topology) => {
            self.set_validator_topology(topology);
        }
    }
}
""",
            "network actor must consume coupled validator dial-roster and topology ownership updates",
        ),
        (
            "p2p_network",
            "peer_connected",
            """
self.validator_dial_scheduler.note_session_established(
    &self.self_id,
    peer.id(),
    tokio::time::Instant::now(),
    self.connect_startup_delay_until,
);
""",
            "accepted peer must publish validator dial session ownership",
        ),
    ):
        items = rust_items(sources[role], item_name)
        if len(items) == 1:
            _require_rust_token_sequence(
                paths[role], items[0], required, description, errors
            )

    config_contracts = (
        (
            "taira_config",
            ("sumeragi", "block"),
            (("max_transactions", 96), ("max_payload_bytes", 16_777_216),
             ("proposal_queue_scan_multiplier", 4)),
            "production Taira profile pins the revision-4 payload ceiling with privacy framing headroom",
        ),
        (
            "taira_config",
            ("sumeragi", "queues"),
            (("authenticated_non_validator_sources", 2), ("body_bytes", 207_618_048),
             ("body_source_bytes", 34_603_008)),
            "production Taira profile pins H=2 and six source partitions",
        ),
        (
            "taira_config",
            ("network",),
            (("max_frame_bytes", 23_068_700), ("max_frame_bytes_block_sync", 23_068_672),
             ("max_frame_bytes_tx_gossip", 13_631_488)),
            "production Taira profile carries maximum privacy transaction and block-sync frames",
        ),
    )
    parsed_configs: dict[str, dict[tuple[str, ...], list[int]]] = {}
    for role in ("taira_config",):
        table: tuple[str, ...] = ()
        values: dict[tuple[str, ...], list[int]] = {}
        for line in sources[role].splitlines():
            section = re.fullmatch(r"\s*\[([A-Za-z0-9_.-]+)\]\s*", line)
            if section:
                table = tuple(section.group(1).split("."))
                continue
            integer = re.fullmatch(
                r"\s*([A-Za-z_][A-Za-z0-9_-]*)\s*=\s*([0-9][0-9_]*)\s*(?:#.*)?",
                line,
            )
            if integer:
                values.setdefault(table + (integer.group(1),), []).append(
                    int(integer.group(2).replace("_", ""))
                )
        parsed_configs[role] = values
    for role, table, expected_values, description in config_contracts:
        observed = {
            field: parsed_configs[role].get(table + (field,), [])
            for field, _value in expected_values
        }
        if any(observed[field] != [value] for field, value in expected_values):
            errors.append(
                f"{paths[role]}: {description} must match exact numeric values "
                f"{dict(expected_values)!r}; found {observed!r}"
            )

    for role, description in (
        ("taira_genesis", "production Taira genesis DA pins the revision-4 protocol ceiling"),
    ):
        try:
            genesis = json.loads(sources[role])
            observed = genesis["sumeragi_v2"]["da_layout"]["max_payload_size_bytes"]
        except (json.JSONDecodeError, KeyError, TypeError) as exc:
            errors.append(f"{paths[role]}: {description} in valid JSON: {exc}")
            continue
        if type(observed) is not int or observed != 16_777_216:
            errors.append(f"{paths[role]}: {description}; found {observed!r}")
    return errors

# Exact comment/literal-free token digests for the allocation-free P2P frame
# geometry used by Sumeragi height activation.  These helpers are part of the
# production refinement boundary: replacing checked arithmetic with a shorter
# estimate can make a locally valid progress envelope impossible to transmit.
_PRODUCTION_P2P_FRAME_GEOMETRY_ITEM_SHA256 = {
    "checked_len_prefixed": (
        "b6411bf29b1e2517fb2c4151c52634334f712f05127c4efd6440166ee6b65207"
    ),
    "peer_id_wire_len_from_raw_key_bytes": (
        "b13f1926dab04641ff700941aaf854a29a0205e4828831da3f207a0930829844"
    ),
    "relay_target_wire_len": (
        "84837f33c9793445071c17cdc11de01ddc1b7b57e061896afd381252743d3c05"
    ),
    "relay_message_wire_payload_len": (
        "993d863fd174abfd9052a1bfcacc9f3c40fca6cd62493b2dbb95222137b579fe"
    ),
    "direct_data_frame_wire_len_from_payload_len": (
        "fa559993ed02666615d9443ef67b6f801f4c52e74d33e39681bed9a283fe2d38"
    ),
    "broadcast_data_frame_wire_len_from_payload_len": (
        "074e4bf40cbdca31a2bd6b5c33bfd0d0a81acec64b33d55954bb42db16fbcf24"
    ),
    "data_frame_wire_len_from_payload_len": (
        "28e6cf55cea091e8229a2aa4b7a1a47e7f5052679dcad8ab029f15dd2beebd78"
    ),
    "validate_transport_queue_geometry": (
        "2b43cba3a15fb667169280663e960cb6fabf5ccde7272d4276b6480041c66632"
    ),
}
_PRODUCTION_P2P_PEER_FRAME_ITEM_SHA256 = {
    "data_message_wire_len_from_payload_len": (
        "d157ece83c8e700725549e91fcb572b61ebe3d8c3e267e9d3bfe31765ff3310c"
    ),
}
_PRODUCTION_P2P_RUN_FRAME_ITEM_SHA256 = {
    "frame_plaintext_cap_for": (
        "4d66d6b2dc3c139c4df54c7c9d7b7640691ce3aa0765a44298b689e63d360e21"
    ),
    "checked_encoded_frame_len": (
        "1355646a778b09fa26e4b9f3de58d3fbce4353845790e12afdd8cf7db7cdd888"
    ),
}
_PRODUCTION_P2P_QUIC_FRAME_ITEM_SHA256 = {
    "try_send": (
        "bd32ba60bc0de89dc1ac9a7062a69c41df4ff6c72dd5c053550385780510891e"
    ),
}
_PRODUCTION_P2P_RECEIVER_FRAME_ITEM_SHA256 = {
    "reserve_for_frame": (
        "cb0e506080c0985c5f81c5c567250e143d072f2f23e06b4072cb3d2b739a8212"
    ),
    "parse_next_encrypted_frame": (
        "88a4e66c149185fa41ac6415c4c24ce864b5cc62f476d1296f0d3a7b3288d055"
    ),
}
_PRODUCTION_P2P_SOURCE_OWNERSHIP_ITEM_SHA256 = {
    "same_owner": (
        "e6bba8d24c683f3b322752c93d977415e41e8dffcd9221f410d462a4006835db"
    ),
    "merge": (
        "36f4915a83458c73f12ff364ebf4ccdd45e6b826fe305e52429098967d26bb55"
    ),
    "source_credits": (
        "f3c9f0c68484f0685560f896d3f782c5c75eb2edf336184ba3ca230bddced095"
    ),
    "extend": (
        "c0d7b44202b40992e33ae23c530fa6c135ff003e23765dde461a044a3a6d8d46"
    ),
}
_PRODUCTION_P2P_SENDER_FRAME_ITEM_SHA256 = {
    "encrypted_frame_geometry": (
        "fa320e44681f1928dd862e8f07b1ea3c72b2c409ba58bf88556c2edd6d57b929"
    ),
    "prepare_message_with_ownership": (
        "2d6ef5bd05a9b9ac31e254546f0e10509df5211daae262d8daa38de8049aa441"
    ),
    "prepare_encoded_buffer": (
        "eb243e718f0b02455dcb31f009ea22aca2f33b4c27aaa00cd309846d87a159fb"
    ),
    "check_queue_limit": (
        "097b47794fe8e9d4ded5357204dfe755f99d476323e11d4c50b604efad0dfb48"
    ),
    "account_enqueued": (
        "dc14a3102e45e8487ed71d3cf7c43af270be5bad47b00fe69fac9f85e51dba13"
    ),
    "enqueue_encrypted": (
        "1efc3f5c8f677f117d82e7e1dcec9c489fbddbcf970b3bba74fd184c5f138421"
    ),
}
_PRODUCTION_P2P_START_FRAME_ITEM_SHA256 = {
    "validate_encrypted_frame_cap": (
        "5305bae9d0febfc2a1348f8f3b9737fb5155a62084a80f731d75bc372ea3bbcd"
    ),
    "start_with_crypto": (
        "5afecc8ebea95afccf327303e006b5e8bbbacb5f42fbe60e3d7b0db1fd42ed8c"
    ),
    "start_with_crypto_and_initial_authorities": (
        "13bf08e9a78d04eb50aa7eddcb834692bbc861767892eb145417bf709805c02f"
    ),
}
_PRODUCTION_P2P_RELIABLE_PEER_ITEM_SHA256 = {
    "post_recover_with_flush_ack": (
        "efe28549106501cb539ea51d42e5c318f5c54bfeb8bef94e1cd1a78a85ea499a"
    ),
    "post_recover_inner": (
        "004f480809bdd5e6f456a5da08b2acd192fb817c44606eec5066141aed2f5b7f"
    ),
    "acknowledge_flush": (
        "b2f9e0921009c50799677284aef793d9a76d184db88b24b1b242eb093e8f71cb"
    ),
    "prepare_owned_or_defer": (
        "0fab3dc65badad08481063c4517f590710ea0592f6e3094d8df05df6a8872f50"
    ),
    "prepare_message_with_ownership": (
        "2d6ef5bd05a9b9ac31e254546f0e10509df5211daae262d8daa38de8049aa441"
    ),
    "prepare_or_defer_with_ownership": (
        "271a4549dc2075166aa9c647a6477f1748f32b536d790dcef967ca9c32087a4d"
    ),
    "retry_deferred": (
        "37298defcaba75e4f3b1e32b55cc6025629e350e2c1a3efe3b358aaad719b3d9"
    ),
    "flush_plain_high": (
        "38da1a9853b5a4c780a3c4dc898ec9ba328b39c1128b7fce7cd2922d2ed5c32b"
    ),
    "flush_plain_low": (
        "bf7e0c4580f6a886ed26d61258f5d8017821b9ab8246ceed1e1f63e510676dd6"
    ),
    "enqueue_current_buffer": (
        "e105440c89db28ab02b220b287af24f04939d31ef0e26bde32cc0346f0450a69"
    ),
    "acknowledge_flushed_batch": (
        "30d441537005fc905493103db122d17670def105f67d36993341038b674a4c63"
    ),
    "fill_batch": (
        "30bba0f4890fd277ee9b22cd433178d840418dbd22a1a615f2a1153857b09ae7"
    ),
    "pop_high_frame": (
        "e7bd48c39d938732bd44cff707989aca7e8d1f55e75c20d559a35ad788302a74"
    ),
    "pop_low_frame": (
        "3d472752bbb83b40712993959290bacdbebe26d604c38500f701642ea62b33a9"
    ),
    "send": (
        "a10e589b0e785b9d113d693e7946a35bc1b6dabb7f8f72c59ff713efe3c393e5"
    ),
    "send_one_ready_stream": (
        "aa54b3d9df965c06ab3b66175859fa31ef5fd377c1bbeacc049a9d8d69a1c7c3"
    ),
    "next_peer_stream_io": (
        "178e60084380d1ac3b9abf5cdcc866748d8de270d215846f85dcc63a7c3b9539"
    ),
    "run": "0b53d79641d9b497ae34691faa4cf2dfeaeed02d28fa58fea00daa6f827cc299",
    "reattach_reply_route": (
        "120803740de09553bb9112a556cceed7e2db414f4f5da3a9691a7886b5264be0"
    ),
}
