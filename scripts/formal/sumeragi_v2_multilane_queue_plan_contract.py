#!/usr/bin/env python3
"""Static QueuePlan bindings for the multilane model gate."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any


def validate_queue_plan_autonomous_only_contract(
    root: Path,
    formal_dir: Path,
    models: Any,
    errors: list[str],
    rust_binding_item: Any,
    regular_file: Any,
    tla_declaration_template: str,
) -> None:
    """Bind QueuePlan execution to the autonomous lane/merge corridor only."""

    if not isinstance(models, list):
        return
    queue_models = [
        model
        for model in models
        if isinstance(model, dict)
        and model.get("module") == QUEUE_PLAN_STARTUP_REPLAY_MODULE
    ]
    if len(queue_models) != 1:
        errors.append(
            "QueuePlan autonomous-only source contract requires exactly one "
            f"{QUEUE_PLAN_STARTUP_REPLAY_MODULE} model"
        )
        return
    production_symbols = queue_models[0].get("production_symbols")
    if not isinstance(production_symbols, list):
        return

    binding_items: dict[tuple[str, str, str], str] = {}
    for relative, kind, symbol, expected_tokens in QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS:
        matches = [
            binding
            for binding in production_symbols
            if isinstance(binding, dict)
            and binding.get("path") == relative
            and binding.get("kind") == kind
            and binding.get("symbol") == symbol
        ]
        if len(matches) != 1:
            errors.append(
                f"{QUEUE_PLAN_STARTUP_REPLAY_MODULE}: autonomous-only binding "
                f"{relative}!{symbol} must occur exactly once, found {len(matches)}"
            )
        else:
            actual_tokens = matches[0].get("required_tokens")
            if (
                not isinstance(actual_tokens, list)
                or tuple(actual_tokens) != expected_tokens
            ):
                errors.append(
                    f"{QUEUE_PLAN_STARTUP_REPLAY_MODULE}: autonomous-only "
                    f"tokens changed for {relative}!{symbol}"
                )

        item = rust_binding_item(
            root,
            relative,
            kind,
            symbol,
            "QueuePlan autonomous-only production binding",
            errors,
        )
        if item is None:
            continue
        binding_items[(relative, kind, symbol)] = item
        for token in expected_tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: QueuePlan autonomous-only item {symbol} "
                    f"is missing source-bound token {token!r}"
                )

    for relative, kind, symbol, tokens in (
        QUEUE_PLAN_AUTONOMOUS_ONLY_ORDERED_SOURCE_CHECKS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "ordered QueuePlan autonomous-only production binding",
                errors,
            )
        if item is None:
            continue
        cursor = -1
        for token in tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{root / relative}: ordered QueuePlan autonomous-only "
                    f"item {symbol} is missing or reorders token {token!r}"
                )
                break
            cursor = position

    for relative, symbol, tokens in QUEUE_PLAN_AUTONOMOUS_ONLY_TEST_BINDINGS:
        item = rust_binding_item(
            root,
            relative,
            "fn",
            symbol,
            "QueuePlan autonomous-only static negative-control test",
            errors,
        )
        if item is None:
            continue
        for token in tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: QueuePlan autonomous-only test {symbol} "
                    f"is missing negative-control token {token!r}"
                )

    module_path = formal_dir / f"{QUEUE_PLAN_STARTUP_REPLAY_MODULE}.tla"
    if regular_file(module_path, "QueuePlan autonomous-only TLA+ module", errors):
        source = module_path.read_text(encoding="utf-8")
        cursor = -1
        for token in QUEUE_PLAN_AUTONOMOUS_ONLY_TLA_ORDERED_TOKENS:
            position = source.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{module_path}: QueuePlan autonomous-only TLA token is "
                    f"missing or reordered: {token!r}"
                )
                break
            cursor = position
        invariant_re = re.compile(
            tla_declaration_template.format(
                symbol=re.escape(QUEUE_PLAN_AUTONOMOUS_ONLY_INVARIANT)
            )
        )
        if invariant_re.search(source) is None:
            errors.append(
                f"{module_path}: missing autonomous-only invariant "
                f"{QUEUE_PLAN_AUTONOMOUS_ONLY_INVARIANT}"
            )

    positive_path = formal_dir / queue_models[0].get("positive_config", "")
    if regular_file(
        positive_path, "QueuePlan autonomous-only positive TLC config", errors
    ):
        marker = f"INVARIANT {QUEUE_PLAN_AUTONOMOUS_ONLY_INVARIANT}\n"
        if positive_path.read_text(encoding="utf-8").count(marker) != 1:
            errors.append(
                f"{positive_path}: autonomous-only invariant must be checked "
                "exactly once"
            )

QUEUE_PLAN_STARTUP_REPLAY_MODULE = "SumeragiV2QueuePlanAdmissionRegistry"

QUEUE_PLAN_AUTONOMOUS_ONLY_INVARIANT = "MLQueuePlanExecutionAutonomousOnly"
QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS = (
    (
        "crates/iroha_core/src/block.rs",
        "fn",
        "external_queue_plan_synced_entrypoint_index",
        (
            "block.external_entrypoints_cloned()",
            ".position(|entrypoint|",
            "entrypoint.admission_intent()",
            "TransactionAdmissionIntent::QueuePlanSynced",
        ),
    ),
    (
        "crates/iroha_core/src/block.rs",
        "fn",
        "validate_staged_execution_controls",
        (
            "bundle.queue_plan_admissions()",
            "if let Some(index) = external_queue_plan_synced_entrypoint_index(block)",
            "must use autonomous lane ownership and a certified merge carrier",
            "reference.is_some() && !native_queue_plan_admissions.is_empty()",
            "native_queue_plan_admissions != state_block.staged_queue_plan_admissions()",
            "staged_merge_entry",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_candidate.rs",
        "method",
        "V2CandidateAssembler::snapshot_routable_candidates",
        (
            "let queue_plan_synced =",
            "TransactionAdmissionIntent::QueuePlanSynced",
            "binding.validate_for_request(",
            "report.routable = report.routable.saturating_add(1)",
            "if queue_plan_synced",
            "report.work_deferred = report.work_deferred.saturating_add(1)",
            "records.push(CandidateRecord {",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::bounded_pending_snapshot",
        (
            "let live_reservations = self.lane_reservations.lock().live_hashes()",
            "let mut live_reservation_fifo_cut = None",
            "for hash in &live_reservations",
            "let order = self.fifo_order_by_hash.get(hash)?",
            "live_reservation_fifo_cut.map_or(order.value().ordinal",
            "if live_reservations.contains(hash) || global_owners.contains_key(hash)",
            "let Some(fifo_order) = self.fifo_order_by_hash.get(hash)",
            "live_reservation_fifo_cut.is_some_and(|cut| fifo_order.value().ordinal >= cut)",
            "blocked_by_fifo_predecessor = true",
        ),
    ),
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "fn",
        "validate_lane_executable_payload_body",
        (
            "entrypoints.is_empty()",
            "entrypoints.iter().any(|entrypoint|",
            "entrypoint.admission_intent() != TransactionAdmissionIntent::QueuePlanSynced",
            "LaneAutonomousArtifactError::InvalidAdmissionIntent",
            "let encoded_payload_body_len = Encode::encode(&(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "&mut V2LaneWorkAdapter::prepare",
        (
            "if !candidates.is_empty()",
            "let broad_autonomous_route_exclusion =",
            "proposal_lookahead_enabled(&nexus, self.context.height)",
            "let is_queue_plan_synced =",
            "== iroha_data_model::transaction::TransactionAdmissionIntent::",
            "QueuePlanSynced;",
            "(is_queue_plan_synced",
            "|| (broad_autonomous_route_exclusion",
            ".contains_key(&(route.lane_id, route.dataspace_id))",
            ".then_some(index)",
            "CandidateWorkUnavailable::new(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::bind_locked_global_body_from_origin",
        (
            "!origin_matches || block.header().height().get() != self.context.height",
            "if crate::block::external_queue_plan_synced_entrypoint_index(block).is_some()",
            "let canonical_recovery = canonical_v2_lane_payload_matches_kura(",
            "retain_pending_certified_merge_entry_for_locked_carrier(",
            "retire_autonomous_payload_batch(&losing_pending)",
        ),
    ),
)

QUEUE_PLAN_AUTONOMOUS_ONLY_ORDERED_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/block.rs",
        "fn",
        "validate_staged_execution_controls",
        (
            "let native_queue_plan_admissions =",
            "external_queue_plan_synced_entrypoint_index(block)",
            "return Err(Self::execution_context_error",
            "reference.is_some() && !native_queue_plan_admissions.is_empty()",
            "native_queue_plan_admissions != state_block.staged_queue_plan_admissions()",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_candidate.rs",
        "method",
        "V2CandidateAssembler::snapshot_routable_candidates",
        (
            "let queue_plan_synced =",
            "binding.validate_for_request(",
            "report.routable = report.routable.saturating_add(1)",
            "if queue_plan_synced",
            "report.work_deferred = report.work_deferred.saturating_add(1)",
            "break;",
            "records.push(CandidateRecord {",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::bounded_pending_snapshot",
        (
            "let live_reservations = self.lane_reservations.lock().live_hashes()",
            "let mut live_reservation_fifo_cut = None",
            "for hash in &live_reservations",
            "let order = self.fifo_order_by_hash.get(hash)?",
            "live_reservation_fifo_cut = Some(",
            "let mut global_owners = self.global_selection_owners.lock()",
            "if live_reservations.contains(hash) || global_owners.contains_key(hash)",
            "let Some(fifo_order) = self.fifo_order_by_hash.get(hash)",
            "live_reservation_fifo_cut.is_some_and(|cut| fifo_order.value().ordinal >= cut)",
            "if self.durability_transition_active(hash)",
        ),
    ),
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "fn",
        "validate_lane_executable_payload_body",
        (
            "if entrypoints.is_empty()",
            "if entrypoints.iter().any(|entrypoint|",
            "entrypoint.admission_intent() != TransactionAdmissionIntent::QueuePlanSynced",
            "return Err(LaneAutonomousArtifactError::InvalidAdmissionIntent)",
            "let encoded_payload_body_len = Encode::encode(&(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "&mut V2LaneWorkAdapter::prepare",
        (
            "let broad_autonomous_route_exclusion =",
            "let is_queue_plan_synced =",
            "(is_queue_plan_synced",
            "|| (broad_autonomous_route_exclusion",
            ".then_some(index)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::bind_locked_global_body_from_origin",
        (
            "if !origin_matches || block.header().height().get() != self.context.height",
            "external_queue_plan_synced_entrypoint_index(block).is_some()",
            "return V2LaneIngressOutcome::Rejected;",
            "let canonical_recovery = canonical_v2_lane_payload_matches_kura(",
            "retain_pending_certified_merge_entry_for_locked_carrier(",
            "retire_autonomous_payload_batch(&losing_pending)",
        ),
    ),
)

QUEUE_PLAN_AUTONOMOUS_ONLY_TEST_BINDINGS = (
    (
        "crates/iroha_core/src/sumeragi/v2_candidate.rs",
        "queue_plan_intent_remains_an_autonomous_fifo_barrier_after_exact_binding",
        (
            "TransactionAdmissionIntent::QueuePlanSynced",
            "vec![queue_plan.clone(), follower.clone()]",
            "install_queue_plan_pending_binding_for_test(&binding)",
            "assert!(bound.is_empty())",
            "assert_eq!(bound_report.work_deferred, 1)",
        ),
    ),
    (
        "crates/iroha_core/src/queue/global_guard_claim_conflict_tests.rs",
        "globally_bound_absent_registry_blocks_selection_and_preserves_exact_fifo",
        (
            "reserve_transactions_for_lane(",
            "assert_eq!(fixture.queue.fifo_snapshot_for_test(), vec![follower_hash])",
            "assert!(predecessor_order < follower_order)",
            ".bounded_pending_snapshot(&fixture.state.view(), nonzero!(2_usize))",
            "assert!(fixture.queue.global_selection_owners.lock().is_empty())",
        ),
    ),
    (
        "crates/iroha_core/src/queue/global_guard_claim_conflict_tests.rs",
        "globally_bound_gossip_waits_for_certificate_and_retains_it_after_exact_marker",
        (
            "iroha_crypto::Algorithm::BlsNormal",
            "QueuePlanGossipAdmission::AwaitingCertificate",
            "persist_pending_queue_plan_admission_certificate(&certificate)",
            "QueuePlanGossipAdmission::Certified(bytes) if bytes.as_slice() == certificate",
            "install_queue_plan_registry_value_for_test(&fixture.state, &fixture.binding)",
            "fixture.transaction_time_to_live + Duration::from_millis(1)",
        ),
    ),
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "autonomous_payload_validator_rejects_ordinary_entrypoint",
        (
            "let (network_id, epoch, mut payload) = autonomous_payload_fixture(&keypairs)",
            "payload.entrypoints[0] = TransactionEntrypoint::External(",
            ".sign(transaction_key.private_key())",
            "payload.validate(network_id, epoch)",
            "Err(LaneAutonomousArtifactError::InvalidAdmissionIntent)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "candidate_provider_anchors_pending_autonomous_payload_and_defers_queue_conflict",
        (
            "planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0)",
            "let conflicting = CandidateDescriptor::new(&accepted, &routing_plan)",
            ".prepare(&context, 0, &[conflicting])",
            ".expect_err(\"ordinary ownership cannot overlap a live lane reservation\")",
            "assert_eq!(unavailable.indices(), &BTreeSet::from([0]))",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/autonomous_retirement_and_merge_tests.rs",
        "assert_locked_external_queue_plan_body_rejects_before_retiring_autonomous_owner",
        (
            "TransactionAdmissionIntent::QueuePlanSynced",
            "mark_global_body_locked_for_block(&mut adapter, &forbidden)",
            "V2LaneIngressOutcome::Rejected",
            "assert_eq!(adapter.pending_autonomous_anchor_payloads, pending_before)",
            "assert_eq!(queue.live_lane_reservations(), reservations_before)",
            "assert_eq!(queue.fifo_snapshot_for_test(), fifo_before)",
        ),
    ),
    (
        "crates/iroha_core/src/block.rs",
        "exact_parent_queue_plan_admission_rejects_ordinary_external_execution",
        (
            "TransactionAdmissionIntent::QueuePlanSynced",
            "validate_queue_plan_ttl_fixture(&fixture)",
            "QueuePlanSynced external execution must be rejected before voting",
            "assert_external_queue_plan_role_rejected(error.as_ref())",
            ".contains_key(&fixture.signed_hash)",
        ),
    ),
    (
        "crates/iroha_core/src/block.rs",
        "assert_external_queue_plan_role_rejected",
        (
            "BlockValidationError::ExecutionContextInvalid(message)",
            'message.contains("must use autonomous lane ownership")',
            "unexpected external QueuePlan rejection",
        ),
    ),
)

QUEUE_PLAN_AUTONOMOUS_ONLY_TLA_ORDERED_TOKENS = (
    'ExecutionRoles == {"None", "Autonomous", "Ordinary"}',
    'executionRole = "None"',
    "executionRole' =",
    'THEN "Ordinary"',
    'ELSE "Autonomous"',
    "MLQueuePlanExecutionAutonomousOnly ==",
    'executedBinding # "None" => executionRole = "Autonomous"',
    "/\\ MLQueuePlanExecutionAutonomousOnly",
)

QUEUE_PLAN_STARTUP_REPLAY_BINDINGS = (
    (
        "crates/iroha_core/src/queue/journal.rs",
        "method",
        "QueuePlanJournalReplay::into_verified_records",
        (
            "self.verify_snapshot_content()?",
            "std::mem::take(&mut self.live_positions)",
            "live.ownership_position",
            "self.verify_snapshot_storage()?",
            "record.claim_digest()",
            "record.entrypoint_hash != entrypoint_hash",
            "record.plan_digest() != live.plan_digest",
            "claim_digest != live.claim_digest",
            "verified.push(record)",
            "Ok(verified)",
        ),
    ),
    (
        "crates/iroha_core/src/queue/journal.rs",
        "method",
        "QueuePlanJournal::remove_all_live_exact_atomic_strict_durable",
        (
            "remove_many_exact_atomic_strict_durable_inner(removals, true)?",
            "Ok(())",
        ),
    ),
    (
        "crates/iroha_core/src/queue/journal.rs",
        "method",
        "QueuePlanJournal::remove_many_exact_atomic_strict_durable_inner",
        (
            "self.ensure_healthy()?",
            "removals.len() > self.limits.max_live_records",
            "QueuePlanJournalFrameV1::RemoveBatch(requested.clone())",
            "prepare_replay_with_removed_entrypoints(Some(&entrypoints))",
            "if require_all_live",
            "live_removals.len() != requested.len()",
            "QueuePlanJournalExactRemoveResult::Removed",
            "atomic live-removal batch contains an already-absent target",
            "QueuePlanJournalFrameV1::RemoveBatch(live_removals.clone())",
            "self.compact(true)?",
            "if compacted != (outcomes.clone(), live_removals.clone())",
            "self.append_encoded(&encoded, AppendPhase::OrdinaryRemove)",
            "self.sync_all_raw(SyncPhase::General)?",
            "Ok(outcomes)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::ensure_plan_journal_replay_startup_shape_locked",
        (
            "self.txs.is_empty()",
            "self.materialized_active_len() == 0",
            "self.materialized_retained_bytes() == 0",
            "self.tx_hashes.is_empty()",
            "self.queued_count.load(Ordering::Acquire) == 0",
            "self.routing_plans.is_empty()",
            "self.durable_plan_claims.is_empty()",
            "self.tx_encoded_len.is_empty()",
            "self.tx_gas_cost.is_empty()",
            "self.tx_enqueued_at_ms.is_empty()",
            "self.queued_tx_enqueued_at_ms.is_empty()",
            "self.queued_age_ring.lock().is_empty()",
            "self.removed_hashes.is_empty()",
            "self.txs_per_user.is_empty()",
            "fee_admission_reservations",
            "self.expiry_ring.lock().is_empty()",
            "self.expiry_ring_members.is_empty()",
            "self.tx_gossip.is_empty()",
            "self.tx_teu.is_empty()",
            "lane_teu_pending",
            "dataspace_teu_pending",
            "only exact durable reservation FIFO identities may pre-exist",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::plan_journal_replay_reservation_shape_locked",
        (
            "store.durable_owned_hashes().collect::<HashSet<_>>()",
            ".filter(|hash| !self.txs.contains_key(hash))",
            "expected_missing_payload_hashes != store.missing_payload_hashes",
            "missing_reservation_payload_count",
            "store.missing_payload_hashes.len()",
            "store.live_by_entrypoint.values().chain(",
            "completed_releases",
            "record.validate()",
            ".insert(hash, record.fifo_order)",
            "multiple durable FIFO owners",
            "durable_owned_hashes",
            "durable_fifo_orders",
            "missing_payload_hashes: store.missing_payload_hashes.clone()",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::prepare_plan_journal_replay_locked",
        (
            "self.ensure_plan_journal_replay_startup_shape_locked()?",
            "self.plan_journal_replay_reservation_shape_locked()?",
            "journal_hashes.len() != records.len()",
            ".missing_payload_hashes",
            ".difference(&journal_hashes)",
            "commit_barrier_hashes.contains(hash)",
            "state_view.transactions.get(&*hash).is_none()",
            "let replay_observed_at = self.time_source.get_unix_time();",
            "AcceptedTransaction::accept_entrypoint_at_time",
            "accepted.hash_as_entrypoint() != entrypoint_hash",
            "queue_plan_replay_reservation_owner",
            "reservation_shape.durable_owned_hashes.contains(&hash)",
            "reservation_owner.is_present()",
            "reservation_owner.fifo_order()",
            "accepted.has_committed_replay_identity(state_view)",
            "state_view.has_entrypoint(entrypoint_hash)",
            "recorded_global_admission_identity",
            "queue_plan_admission_registry_match_in_view",
            "queue_plan_admission_registry_match",
            "QueuePlanAdmissionRegistryMatch::Absent",
            "QueuePlanAdmissionRegistryMatch::Conflict",
            "has_materialized_owner || has_durable_reservation_owner",
            "tombstoned_conflicting_global_admission",
            "QueuePlanBindingApplicationEvidence::AppliedDirect",
            "QueuePlanBindingApplicationEvidence::AppliedViaSignedAlias",
            "queue_plan_binding_application_evidence_in_view",
            "evidence == expected_evidence",
            "global_registry_match.is_none()",
            "self.is_expired_at_with_enqueue_timestamp(",
            "replay_observed_at",
            "!has_durable_reservation_owner",
            "resolve_routing_plan_for_queue_admission(",
            "durable_plan_claim_context_revalidates_in_view",
            "QueueAdmissionPreparationMode::AtomicJournalReplay",
            "transaction_selection_durability_faulted()",
            "self.active_len()",
            "self.retained_bytes()",
            "projected_active > self.capacity.get()",
            "projected_retained > self.max_retained_bytes.get()",
            "projected > self.capacity_per_user.get()",
            ".reserve(admission.hash, reservation)",
            "orphaned FIFO identity",
            "reservation FIFO anchors disagree with authenticated journal order",
            "anchors.len() != reservation_shape.durable_fifo_orders.len()",
            "final_fifo.len() > self.tx_hashes.capacity()",
            "terminal_removals",
            "terminal_removals.push",
            "Ok(PreparedQueuePlanReplay {",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::reject_exact_queue_plan_admission_claim",
        ("reject_exact_queue_plan_admission_claim_inner(binding, false)",),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::reject_unreserved_replay_terminal_queue_plan_admission_claim",
        ("reject_exact_queue_plan_admission_claim_inner(binding, true)",),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::reject_exact_queue_plan_admission_claim_inner",
        (
            ".validate_structure()",
            "self.push_remove_lock.lock()",
            "self.durability_transition_active(&hash)",
            "self.wait_for_durability_transitions(&[hash])",
            ".durable_plan_claims",
            "indexed_binding != binding",
            "if require_unreserved_replay_terminal_owner",
            "reservations.live_by_entrypoint.contains_key(&hash)",
            ".commit_barriers",
            ".plan_tombstoned",
            "reservations.release_barriers",
            "reservations.completed_releases",
            "self.global_selection_owners.lock().contains_key(&hash)",
            "self.inflight_guards.load(Ordering::Acquire) != 0",
            "self.selection_attempts.load(Ordering::Acquire) != 0",
            ".begin_durability_transition_locked([hash])",
            "self.tombstone_conflicting_global_admission(binding)?",
            "self.finalize_conflicting_global_admission_locked(",
            "return Ok(true)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::remove_state_committed_replay_owners_preserving_globally_bound",
        (
            "has_committed_replay_identity(state_view)",
            "CommittedHashCleanupMode::PreserveGloballyBoundOwners",
            "has_globally_bound_durable_claim(carrier_hash)",
            "QueuePlanBindingApplicationEvidence::AppliedDirect",
            "QueuePlanBindingApplicationEvidence::AppliedViaSignedAlias",
            "global_admission_binding()",
            "queue_plan_admission_registry_match_in_view",
            "registry_match == QueuePlanAdmissionRegistryMatch::Exact",
            "queue_plan_binding_application_evidence_in_view",
            "evidence == expected_evidence",
            "replay_terminal_bindings.push(binding)",
            "reject_unreserved_replay_terminal_queue_plan_admission_claim(&binding)?",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::apply_plan_journal_replay_locked",
        (
            "terminal_removals: _",
            "*self.fee_admission_reservations.lock() = fee_reservations;",
            "*self.next_fifo_ordinal.lock() = next_fifo_ordinal;",
            "self.fifo_order_by_hash.insert(hash, fifo_order);",
            "self.txs.insert(hash, Arc::clone(&tx_arc));",
            "self.track_active_transaction();",
            "let lane_id = routing_decision.lane_id;",
            "self.routing_plans.insert(hash, routing_plan.clone());",
            "self.durable_plan_claims.insert(hash, claim.clone());",
            "self.track_expiry_hash(hash);",
            "notifications.push(QueueAdmissionNotification {",
            "self.apply_per_user_tx_count_increments(per_user_increments);",
            "self.reconcile_missing_reservation_payloads_locked(&mut store);",
            "self.replace_fifo_locked(&final_fifo);",
            "(summary, notifications)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::replay_plan_journal",
        (
            "self.plan_journal_install_lock.lock()",
            "self.lane_reservation_transition_lock.lock()",
            "state.lock_lane_lifecycle_work_admission()",
            "state.state_view_generation()",
            "let state_view = state.view();",
            "self.ensure_plan_journal_replay_startup_shape_locked()?",
            "self.sync_nexus_routing_with_view(&state_view);",
            "let mut journal_guard = self.plan_journal.lock();",
            "let queue_guard = self.push_remove_lock.lock();",
            "let records = journal.prepare_replay()?.into_verified_records()?;",
            "let expected_record_claims = records",
            "self.prepare_plan_journal_replay_locked(",
            "let observed_record_claims = journal",
            "if observed_record_claims != expected_record_claims",
            "let terminal_removals = prepared.terminal_removals.clone();",
            "remove_all_live_exact_atomic_strict_durable(&terminal_removals)",
            "self.mark_plan_journal_durability_fault",
            "self.apply_plan_journal_replay_locked(prepared)",
            "self.publish_admission_notifications(&notifications);",
            "self.publish_backpressure_state(self.active_len(), backpressure_telemetry);",
            "status::set_tx_queue_pressure(self.pressure_snapshot());",
            "Ok(summary)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::complete_lane_reservation_startup_reconciliation",
        (
            "self.lane_reservation_transition_lock.lock()",
            "self.push_remove_lock.lock()",
            "self.transaction_selection_durability_faulted()",
            "!store.commit_barriers.is_empty()",
            "!store.release_barriers.is_empty()",
            "!store.completed_releases.is_empty()",
            "!store.missing_payload_hashes.is_empty()",
            "lane_reservation_reconciliation_pending",
            ".store(false, Ordering::Release)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::lane_reservation_reconciliation_snapshot",
        (
            "self.transaction_selection_durability_faulted()",
            "self.lane_reservation_transition_lock.lock()",
            "self.push_remove_lock.lock()",
            "self.lane_reservation_journal.lock().is_none()",
            "LaneQueueReservationError::JournalNotInstalled",
            "self.lane_reservation_reconciliation_snapshot_locked()",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::lane_reservation_reconciliation_snapshot_locked",
        (
            "let store = self.lane_reservations.lock()",
            "store.live_by_entrypoint.values()",
            "self.validate_live_reservation_against_queue(record)?",
            "LaneQueueReservationError::ReconciliationFifoOrderMismatch",
            "LaneQueueReservationError::ReconciliationMissingDurableClaim",
            "reconciliation_record_from_durable_claim",
            "store.commit_barriers.clone()",
            "store.release_barriers.clone()",
            "store.completed_releases.clone()",
            "key.validate()",
            "for barrier in &prepared_release_barriers",
            "barrier\n                .validate()",
            "for completion in &completed_releases",
            "completion\n                .validate()",
            "commit_barriers.sort_by_key",
            "prepared_release_barriers.sort_by_key",
            "completed_releases.sort_by_key",
            "let ordered_owner_phases = self.lane_reservation_recovery_phases_locked()?;",
            "ordered_records.sort_by_key",
            "LaneQueueReservationError::ReconciliationDuplicateFifoOrdinal",
            "MAX_LANE_EXECUTABLE_ENTRYPOINTS",
            "Ok(LaneQueueReservationReconciliationSnapshotV1 {",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "plan_lane_reservation_ownership",
        (
            "let current_snapshot = queue.lane_reservation_reconciliation_snapshot()?;",
            "handoff.into_queue_handoff()",
            "revalidate_lane_reservation_startup_reconciliation_receipt(",
            "deferred_terminal_recovery",
            "if snapshot.is_empty()",
            "let release_barriers = snapshot.release_barriers()",
            "let commit_barriers = snapshot.commit_barriers.as_slice()",
            "has_committed_entrypoint",
            "unique_recovered",
            "get_merge_entry_by_carrier_height",
            "authenticated_autonomous_carrier_application_projections",
            "certified_merge_queue_reservations",
            "exact_committed_carrier_height_for_group",
            "authenticated_committed_carriers",
            ".queue_cleanup_authorization()",
            "commit_authorization",
            "ReservationReconciliationAction::Commit {",
            "lane_incarnation_at_height",
            "classify_autonomous_lane_reservation_groups",
            "canonical_autonomous_carrier_disposition",
            "RecoverCanonicalBodies",
            "InstallHistoricalAutonomousRecoveries",
            "if queue.lane_reservation_reconciliation_snapshot()? != snapshot",
            "let replay_receipt = match recovered_receipt",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "apply_lane_reservation_reconciliation_plan",
        (
            "replay_receipt",
            "deferred_terminal_recovery",
            "historical_autonomous_install_is_durable",
            "revalidate_lane_reservation_startup_reconciliation_receipt(&replay_receipt, &snapshot)",
            "ReservationReconciliationAction::Commit {",
            "finalize_startup_committed_canonical_carriers(",
            "retire_autonomous_lane_slot_and_release_reservations",
            "release_strictly_absent_lane_reservations_in_order",
            "let final_snapshot = queue.lane_reservation_reconciliation_snapshot()?;",
            "!final_snapshot.commit_barriers.is_empty()",
            "!final_snapshot.prepared_release_barriers.is_empty()",
            "!final_snapshot.completed_releases.is_empty()",
            "complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions(",
            "queue.complete_lane_reservation_startup_reconciliation(replay_receipt)?;",
        ),
    ),
    (
        "crates/irohad/src/main.rs",
        "method",
        "Iroha::start_with_runtime_deps",
        (
            "install_lane_reservation_journal(",
            "install_plan_journal(",
            "replay_plan_journal(&state)",
            "IrohaNetwork::start_with_crypto_and_initial_trusted_sources(",
        ),
    ),
)
QUEUE_PLAN_STARTUP_REPLAY_ORDERED_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/queue/journal.rs",
        "method",
        "QueuePlanJournalReplay::into_verified_records",
        (
            "self.verify_snapshot_content()?;",
            "std::mem::take(&mut self.live_positions)",
            "ordered.sort_unstable_by_key",
            "for (entrypoint_hash, live) in ordered {",
            "self.verify_snapshot_storage()?;",
            "let claim_digest = record.claim_digest()",
            "if record.entrypoint_hash != entrypoint_hash",
            "verified.push(record);",
            "self.verify_snapshot_content()?;",
            "Ok(verified)",
        ),
    ),
    (
        "crates/iroha_core/src/queue/journal.rs",
        "method",
        "QueuePlanJournal::remove_many_exact_atomic_strict_durable_inner",
        (
            "let (outcomes, live_removals) =",
            "if require_all_live",
            "atomic live-removal batch contains an already-absent target",
            "if live_removals.is_empty()",
            "let encoded = encode_frame(",
            "self.ensure_append_capacity(encoded.len())",
            "self.compact(true)?;",
            "let compacted =",
            "if compacted != (outcomes.clone(), live_removals.clone())",
            "self.append_encoded(&encoded, AppendPhase::OrdinaryRemove)",
            "self.sync_all_raw(SyncPhase::General)?;",
            "Ok(outcomes)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::prepare_plan_journal_replay_locked",
        (
            "self.ensure_plan_journal_replay_startup_shape_locked()?;",
            "self.plan_journal_replay_reservation_shape_locked()?;",
            "let journal_hashes = records",
            ".missing_payload_hashes",
            ".difference(&journal_hashes)",
            "let replay_observed_at = self.time_source.get_unix_time();",
            "for record in records {",
            "AcceptedTransaction::accept_entrypoint_at_time(",
            "queue_plan_replay_reservation_owner(",
            "let state_committed = accepted.has_committed_replay_identity(state_view);",
            "let carrier_committed = state_view.has_entrypoint(entrypoint_hash);",
            "let global_registry_match = if let Some(binding) = global_binding.as_ref() {",
            "if state_committed\n                && matches!(",
            "QueuePlanAdmissionRegistryMatch::Absent\n                            | QueuePlanAdmissionRegistryMatch::Conflict",
            "if has_materialized_owner || has_durable_reservation_owner {",
            "terminal_removals.push((",
            "if state_committed && let Some(binding) = global_binding.as_ref() {",
            "QueuePlanBindingApplicationEvidence::AppliedDirect",
            "QueuePlanBindingApplicationEvidence::AppliedViaSignedAlias",
            "State::queue_plan_binding_application_evidence_in_view(state_view, &binding)",
            "Ok(evidence) if evidence == expected_evidence",
            "if state_committed && !has_durable_reservation_owner {",
            "let canonical_pending_handoff = if !state_committed",
            "self.is_expired_at_with_enqueue_timestamp(",
            "resolve_routing_plan_for_queue_admission(",
            "prepare_checked_for_enqueue(",
            "if self.transaction_selection_durability_faulted()",
            "let mut projected_active = self.active_len();",
            "let mut fifo_orders =",
            "let anchors = pending_admissions",
            "Ok(PreparedQueuePlanReplay {",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::reject_exact_queue_plan_admission_claim",
        ("self.reject_exact_queue_plan_admission_claim_inner(binding, false)",),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::reject_unreserved_replay_terminal_queue_plan_admission_claim",
        ("self.reject_exact_queue_plan_admission_claim_inner(binding, true)",),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::reject_exact_queue_plan_admission_claim_inner",
        (
            "binding\n            .validate_structure()",
            "let queue_guard = self.push_remove_lock.lock();",
            "if self.durability_transition_active(&hash) {",
            "self.wait_for_durability_transitions(&[hash]);",
            "let Some(indexed_claim) = self",
            "if &indexed_binding != binding {",
            "if require_unreserved_replay_terminal_owner {",
            "let reservation_owned = {",
            "reservations.live_by_entrypoint.contains_key(&hash)",
            ".commit_barriers",
            ".plan_tombstoned",
            "reservations.release_barriers.iter().any",
            "reservations.completed_releases.iter().any",
            "if reservation_owned",
            "self.global_selection_owners.lock().contains_key(&hash)",
            "self.inflight_guards.load(Ordering::Acquire) != 0",
            "self.selection_attempts.load(Ordering::Acquire) != 0",
            "let transaction = self",
            ".begin_durability_transition_locked([hash])",
            "self.tombstone_conflicting_global_admission(binding)?;",
            "self.finalize_conflicting_global_admission_locked(",
            "return Ok(true);",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::remove_state_committed_replay_owners_preserving_globally_bound",
        (
            ".has_committed_replay_identity(state_view)",
            "CommittedHashCleanupMode::PreserveGloballyBoundOwners",
            "self.has_globally_bound_durable_claim(carrier_hash)",
            "let expected_evidence = if state_view.has_entrypoint(carrier_hash) {",
            "QueuePlanBindingApplicationEvidence::AppliedDirect",
            "QueuePlanBindingApplicationEvidence::AppliedViaSignedAlias",
            "let binding = claim",
            ".global_admission_binding()",
            "State::queue_plan_admission_registry_match_in_view(",
            "if registry_match == QueuePlanAdmissionRegistryMatch::Exact {",
            "State::queue_plan_binding_application_evidence_in_view(state_view, &binding)",
            "Ok(evidence) if evidence == expected_evidence",
            "replay_terminal_bindings.push(binding);",
            "for binding in replay_terminal_bindings {",
            "self.reject_unreserved_replay_terminal_queue_plan_admission_claim(&binding)?",
            "Ok(removed)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::apply_plan_journal_replay_locked",
        (
            "*self.fee_admission_reservations.lock() = fee_reservations;",
            "*self.next_fifo_ordinal.lock() = next_fifo_ordinal;",
            "for replayed in admissions {",
            "self.fifo_order_by_hash.insert(hash, fifo_order);",
            "self.txs.insert(hash, Arc::clone(&tx_arc));",
            "self.durable_plan_claims.insert(hash, claim.clone());",
            "notifications.push(QueueAdmissionNotification {",
            "self.apply_per_user_tx_count_increments(per_user_increments);",
            "self.reconcile_missing_reservation_payloads_locked(&mut store);",
            "self.replace_fifo_locked(&final_fifo);",
            "(summary, notifications)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::replay_plan_journal",
        (
            "self.plan_journal_install_lock.lock()",
            "self.lane_reservation_transition_lock.lock()",
            "state.lock_lane_lifecycle_work_admission()",
            "let state_view = state.view();",
            "self.ensure_plan_journal_replay_startup_shape_locked()?;",
            "self.sync_nexus_routing_with_view(&state_view);",
            "let mut journal_guard = self.plan_journal.lock();",
            "let queue_guard = self.push_remove_lock.lock();",
            "let records = journal.prepare_replay()?.into_verified_records()?;",
            "let expected_record_claims = records",
            "let prepared = self.prepare_plan_journal_replay_locked(",
            "let observed_record_claims = journal",
            ".prepare_replay()?",
            ".into_verified_records()?",
            "if observed_record_claims != expected_record_claims",
            "let terminal_removals = prepared.terminal_removals.clone();",
            "remove_all_live_exact_atomic_strict_durable(&terminal_removals)",
            "self.apply_plan_journal_replay_locked(prepared)",
            "self.publish_admission_notifications(&notifications);",
            "Ok(summary)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::lane_reservation_reconciliation_snapshot",
        (
            "if self.transaction_selection_durability_faulted()",
            "let _reservation_transition_guard = self.lane_reservation_transition_lock.lock();",
            "let _queue_guard = self.push_remove_lock.lock();",
            "if self.transaction_selection_durability_faulted()",
            "if self.lane_reservation_journal.lock().is_none()",
            "self.lane_reservation_reconciliation_snapshot_locked()",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::lane_reservation_reconciliation_snapshot_locked",
        (
            "let store = self.lane_reservations.lock();",
            "for record in store.live_by_entrypoint.values() {",
            "self.validate_live_reservation_against_queue(record)?;",
            "let mut commit_barriers = store.commit_barriers.clone();",
            "commit_barriers.sort_by_key",
            "let mut prepared_release_barriers = store.release_barriers.clone();",
            "prepared_release_barriers.sort_by_key",
            "let mut completed_releases = store.completed_releases.clone();",
            "completed_releases.sort_by_key",
            "drop(store);",
            "let ordered_owner_phases = self.lane_reservation_recovery_phases_locked()?;",
            "ordered_records",
            "Ok(LaneQueueReservationReconciliationSnapshotV1 {",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "plan_lane_reservation_ownership",
        (
            "let current_snapshot = queue.lane_reservation_reconciliation_snapshot()?;",
            "let (snapshot, recovered_receipt, deferred_terminal_recovery) = match lifecycle_handoff",
            "handoff.into_queue_handoff()",
            "if snapshot != current_snapshot",
            "revalidate_lane_reservation_startup_reconciliation_receipt(",
            "if snapshot.is_empty() {",
            "let release_barriers = snapshot.release_barriers();",
            "let commit_barriers = snapshot.commit_barriers.as_slice();",
            "for barrier in &release_barriers {",
            "for key in commit_barriers {",
            "let mut authenticated_committed_carriers =",
            "BTreeMap::<",
            "for input in inputs.iter_mut().filter(|input| input.committed) {",
            ".get_merge_entry_by_carrier_height(carrier_height)?",
            "authenticated_autonomous_carrier_application_projections(",
            "let reservation_group =",
            "lane_queue_reservation_group_binding_from_ordered_keys(",
            "input.commit_authorization = Some(",
            ".queue_cleanup_authorization()",
            "for input in &mut inputs {",
            "let authorization = input.commit_authorization.take().ok_or_else(||",
            "actions.push(ReservationReconciliationAction::Commit {",
            "if queue.lane_reservation_reconciliation_snapshot()? != snapshot",
            "let replay_receipt = match recovered_receipt",
            "LaneReservationReconciliationPlan {",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "apply_lane_reservation_reconciliation_plan",
        (
            "historical_autonomous_install_is_durable",
            "revalidate_lane_reservation_startup_reconciliation_receipt(&replay_receipt, &snapshot)",
            "let mut authorized_commit_groups = Vec::new();",
            "for action in actions {",
            "ReservationReconciliationAction::Commit {",
            "authorized_commit_groups.push((keys, authorization))",
            "finalize_startup_committed_canonical_carriers(",
            "for action in remaining_actions {",
            "retire_autonomous_lane_slot_and_release_reservations(",
            "queue.release_strictly_absent_lane_reservations_in_order(",
            "let final_snapshot = queue.lane_reservation_reconciliation_snapshot()?;",
            "!final_snapshot.commit_barriers.is_empty()",
            "!final_snapshot.prepared_release_barriers.is_empty()",
            "!final_snapshot.completed_releases.is_empty()",
            "complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions(",
            "queue.complete_lane_reservation_startup_reconciliation(replay_receipt)?;",
        ),
    ),
    (
        "crates/irohad/src/main.rs",
        "method",
        "Iroha::start_with_runtime_deps",
        (
            "install_lane_reservation_journal(",
            "install_plan_journal(",
            "replay_plan_journal(&state)",
            "IrohaNetwork::start_with_crypto_and_initial_trusted_sources(",
        ),
    ),
)
QUEUE_PLAN_STARTUP_REPLAY_FORBIDDEN_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::apply_plan_journal_replay_locked",
        (
            "?",
            "Result<",
            "return Err(",
            "expect(",
            "unwrap(",
            "panic!(",
            "unreachable!(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "plan_lane_reservation_ownership",
        (
            "live_lane_reservations()",
            "lane_reservation_commit_barriers()",
            "lane_reservation_release_barriers()",
            "AutonomousLaneQueueCarrierCleanupAuthorization::from_projection_for_test",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "apply_lane_reservation_reconciliation_plan",
        (
            "lane_reservation_commit_barriers()",
            "lane_reservation_release_barriers()",
            "queue.commit_lane_reservation_group(",
            "commit_lane_reservation_group_with_authorization(",
        ),
    ),
)
QUEUE_PLAN_STARTUP_REPLAY_POST_APPLY_MARKER = (
    "let (summary, notifications) = self.apply_plan_journal_replay_locked(prepared);"
)
QUEUE_PLAN_STARTUP_REPLAY_POST_APPLY_FORBIDDEN_TOKENS = (
    "?",
    "return Err(",
    ".map_err(",
    "expect(",
    "unwrap(",
    "panic!(",
    "unreachable!(",
)
QUEUE_PLAN_STARTUP_REPLAY_TEST_BINDINGS = (
    (
        "crates/iroha_core/src/queue/journal.rs",
        "exact_atomic_live_tombstone_batch_rejects_retry_before_append",
        (
            "remove_all_live_exact_atomic_strict_durable(",
            "expect_err(",
            "io::ErrorKind::InvalidData",
            "the startup publication form must reject a mixed absent and live batch",
            "the all-live precondition must reject a mixed batch before append",
            "rejecting a mixed batch must retain its still-live member",
            "the all-live precondition must reject before another frame is appended",
        ),
    ),
    (
        "crates/iroha_core/src/queue/plan_journal_replay_tests.rs",
        "materialized_replay_rejects_later_record_corruption_before_any_callback",
        (
            ".get_mut(&second_key)",
            ".for_each_record(",
            "expect_err(",
            "callbacks, 0",
            "a valid earlier record must remain private when a later record is corrupt",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "queue_plan_journal_replay_retains_current_admission_rejection_and_fails_startup",
        (
            "expect_err(\"a current admission failure must abort startup\")",
            "failed current admission",
            "assert_eq!(replay_queue.active_len(), 0);",
            "live_record_count()",
            "without publishing or tombstoning a prefix",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "queue_plan_journal_replay_rejects_aggregate_per_user_overflow_without_prefix",
        (
            "capacity_per_user = nonzero!(1_usize)",
            "aggregate per-user overflow must reject the complete replay",
            "std::io::ErrorKind::PermissionDenied",
            "assert_eq!(replay_queue.active_len(), 0);",
            "live_record_count()",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "queue_plan_journal_replay_rejects_orphaned_startup_fifo_identity",
        (
            "fifo_order_by_hash.insert(orphan, fifo_order)",
            "an unowned startup FIFO identity must fail closed",
            "orphaned FIFO identity",
            "Some(fifo_order)",
        ),
    ),
    (
        "crates/iroha_core/src/queue/lane_reservation_tests.rs",
        "reservation_restart_fits_ordinary_fifo_around_middle_anchor",
        (
            "install_lane_reservation_journal(&reservation_path",
            "replay_plan_journal(&state)",
            "Some(u64::try_from(index)",
            "release_lane_reservations_in_order(&[reserved_key])",
            "restart replay must preserve A/B/C",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_recovery_tests.rs",
        "state_committed_live_reservation_replays_quarantined_until_explicit_proof_commit",
        (
            "authenticate and quarantine the sole payload source",
            "tombstoned_committed: 0",
            "assert!(queue.txs.contains_key(&hash));",
            "assert_eq!(queue.queued_len(), 0);",
            "missing_reservation_payload_count",
            "live_record_count()",
            "commit_lane_reservation_for_test(&key)",
            "lane_reservation_commit_barriers().is_empty()",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_recovery_tests.rs",
        "expired_live_reservation_replays_payload_without_fifo_or_tombstone",
        (
            "transaction_time_to_live: Duration::from_millis(1)",
            "time_handle.advance(Duration::from_millis(2));",
            "materialize expired payload under its durable reservation owner",
            "tombstoned_expired: 0",
            "assert_eq!(queue.queued_len(), 0);",
            "must not tombstone the sole payload source",
        ),
    ),
    (
        "crates/iroha_core/src/queue/lane_reservation_tests.rs",
        "restart_reconciliation_snapshot_is_fifo_group_complete_and_read_only",
        (
            "lane_reservation_reconciliation_snapshot()",
            "snapshot.commit_barriers.is_empty()",
            "snapshot.prepared_release_barriers.is_empty()",
            "snapshot.completed_releases.is_empty()",
            "snapshot.release_barriers().is_empty()",
            "assert_eq!(capture_store(), store_before);",
        ),
    ),
    (
        "crates/iroha_core/src/queue/lane_reservation_tests.rs",
        "ordered_release_restart_retains_barrier_until_explicit_evidence_gated_finalize",
        (
            "for crash_after_completion in [false, true]",
            "lane_reservation_reconciliation_snapshot()",
            "reconciliation_snapshot.prepared_release_barriers",
            "reconciliation_snapshot.completed_releases[0]",
            ".ordered_records",
            "reconciliation_snapshot.release_barriers()",
        ),
    ),
    (
        "crates/iroha_core/src/queue/lane_reservation_tests.rs",
        "reservation_group_forget_prefix_replays_and_resumes_exactly_once",
        (
            "assert_eq!(replay.commit_barriers, 1);",
            "lane_reservation_reconciliation_snapshot()",
            "reconciliation_snapshot.commit_barriers, vec![keys[2]]",
            "reconciliation_snapshot",
            ".prepared_release_barriers",
            "reconciliation_snapshot.completed_releases.is_empty()",
        ),
    ),
)
