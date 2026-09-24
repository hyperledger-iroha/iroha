#!/usr/bin/env python3
"""Static QueuePlan bindings for the multilane model gate."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from sumeragi_v2_multilane_geometry_evidence_contract import _code
from sumeragi_v2_multilane_reviewed_rust_source import _mask_rust_comments


# The retry and capacity gates share this physical deadline owner. Keep its
# entire reviewed ledger tuple identical in both gates: neither may replace the
# other's predicates when they bind the same production symbol.
QUEUE_PLAN_PROXY_DEADLINE_BINDING = (
    "crates/iroha_torii/src/lib.rs",
    "fn",
    "execute_incoming_torii_proxy_request_with_admission",
    (
        "let budget_observed_at = tokio::time::Instant::now();",
        "validate_torii_proxy_deadline(proxy_request.deadline_unix_ms)",
        ".checked_sub(TORII_PROXY_RESPONSE_EGRESS_RESERVE)",
        ".filter(|budget| !budget.is_zero())",
        "return queue_plan_capacity_wait::deadline_response(&proxy_request.request, error);",
        "let remaining_budget = absolute_budget.min(TORII_PROXY_EXECUTION_BUDGET);",
        "let deadline = budget_observed_at + remaining_budget;",
        "match tokio::time::timeout_at(\n"
        "        deadline,\n"
        "        execute_incoming_torii_proxy_request_with_admission_inner(\n"
        "            app,\n"
        "            proxy_request,\n"
        "            immediate_sender_peer_id,\n"
        "            pre_admitted_fanout,\n"
        "            proxy_memory,\n"
        "            deadline,\n"
        "        ),\n"
        "    )",
    ),
)


# Both canonical checks around the capacity wait consume this exact deadline.
QUEUE_PLAN_PROXY_CANONICAL_RESPONSE = (
    "if let Some(response) = canonical_queue_plan_synced_response(\n"
    "                app,\n"
    "                &authenticated,\n"
    "                &admission_binding,\n"
    "                ingress_plan.coordinator_route(),\n"
    "                proxy_memory.as_ref(),\n"
    "                execution_deadline,\n"
    "            ) {\n"
    "                return response;\n"
    "            }"
)


def validate_queue_plan_proxy_deadline_owner(item: str, errors: list[str]) -> None:
    """Both gates retain one clock observation before validation and hashing."""
    symbol = QUEUE_PLAN_PROXY_DEADLINE_BINDING[2]
    code = _code(item)
    cursor = 0
    for relation in (
        "let budget_observed_at = tokio::time::Instant::now();",
        "validate_torii_proxy_deadline(",
        "checked_sub(TORII_PROXY_RESPONSE_EGRESS_RESERVE)",
        ".filter(|budget| !budget.is_zero())",
        "let remaining_budget = absolute_budget.min(TORII_PROXY_EXECUTION_BUDGET);",
        "let deadline = budget_observed_at + remaining_budget;",
        "timeout_at(",
    ):
        normalized = _code(relation)
        found = code.find(normalized, cursor)
        if found < 0:
            errors.append(f"{symbol}: canonical QueuePlan deadline order changed: {relation!r}")
            break
        cursor = found + len(normalized)
    observation = code.find(_code("let budget_observed_at = tokio::time::Instant::now();"))
    for setup in ("let request_id = proxy_request.request_id.clone();", "let queue_plan_identity ="):
        position = code.find(_code(setup))
        if observation < 0 or position <= observation:
            errors.append(f"{symbol}: canonical QueuePlan deadline order changed: {setup!r}")
    # Retain whitespace for Rust identifier boundaries. Compact-token counting
    # misses valid typed or mutable shadows that keep the original declaration.
    masked = _mask_rust_comments(item)
    for original in ("budget_observed_at", "absolute_budget", "remaining_budget", "deadline"):
        declarations = re.findall(
            rf"\blet\s+(?:(?:ref|mut)\s+)*(?:r#)?{original}\b", masked
        )
        if len(declarations) != 1:
            errors.append(f"{symbol}: canonical QueuePlan deadline owner is rebound: {original}")


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

    validate_direct_release_authority_contract(root, errors, rust_binding_item)

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

    validate_current_queue_plan_selection(binding_items, errors)
    validate_retained_queue_plan_route_authority(binding_items, errors)
    validate_canonical_queue_plan_retry(binding_items, errors)
    validate_queue_plan_replay_terminal_custody(binding_items, errors)

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
    ('crates/iroha_data_model/src/block/lane_admission.rs',
     'struct',
     'QueuePlanAdmissionBindingV1',
     ('version',
      'network_id_digest',
      'request_id',
      'entrypoint_hash',
      'routing_plan_digest',
      'admission_context',
      'enqueue_timestamp_ms',
      'journal_record_digest')),
    ('crates/iroha_core/src/torii_proxy.rs',
     'fn',
     'validate_queue_plan_binding_for_request',
     ('binding.network_id_digest != queue_plan_admission_network_id_digest(network_id)',
      'binding.request_id != queue_plan_synced_request_id(network_id, transaction.hash())',
      'validate_queue_plan_binding_for_transaction_and_plan(binding, transaction, routing_plan)')),
    ('crates/iroha_core/src/torii_proxy.rs',
     'fn',
     'validate_queue_plan_binding_for_transaction_and_plan',
     ('binding.validate_structure()?;',
      'binding.entrypoint_hash != transaction.hash()',
      'binding.signed_transaction_hash != crate::tx::exact_signed_transaction_hash(transaction)',
      'binding.routing_plan_digest != routing_plan.digest()',
      '.validate_for_routing_plan(routing_plan)?;',
      'crate::queue::queue_plan_journal_record_claim_digest(',
      'transaction.clone(),',
      'routing_plan.clone(),',
      'binding.admission_context.clone(),',
      'binding.enqueue_timestamp_ms,',
      'Some(binding.global_admission_identity()),',
      'if exact_digest != binding.journal_record_digest',
      'Ok(())')),
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
    ('crates/iroha_core/src/sumeragi/v2_candidate.rs',
     'method',
     'V2CandidateAssembler::snapshot_routable_candidates',
     ('let mut queue_plan_barrier = false;',
      'if queue_plan_barrier && !exact_height_lifecycle_transaction(context, &transaction)',
      'let queue_plan_synced =',
      'TransactionAdmissionIntent::QueuePlanSynced',
      'Ok(None) => {',
      'queue_plan_barrier = true;',
      'if queue_plan_barrier',
      'exact_height_lifecycle_candidate(',
      'crate::torii_proxy::validate_queue_plan_binding_for_request(',
      'report.routable = report.routable.saturating_add(1)',
      'if queue_plan_synced',
      'report.work_deferred = report.work_deferred.saturating_add(1)',
      'records.push(CandidateRecord {')),
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
    ('crates/iroha_core/src/sumeragi/v2_lane_work.rs',
     'method',
     '&mut V2LaneWorkAdapter::prepare',
     ('if context != &self.context',
      'match self.refresh_merge_candidates(view)',
      'let reserved_routes = self',
      '.pending_autonomous_anchor_payloads',
      '.map(|key| (key.lane_id, key.dataspace_id))',
      'let reserved_entrypoints = self',
      '.flat_map(|payload| payload.entrypoint_hashes.iter().copied())',
      'candidate.transaction().entrypoint().admission_intent()',
      '== iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced)',
      '.then_some(index)',
      'if !unavailable.is_empty()',
      'QueuePlanSynced work requires its globally admitted autonomous reservation',
      'let route_conflict = candidate.routing_plan().legs().iter().any(|leg| {',
      'reserved_routes.contains(&(leg.route.lane_id, leg.route.dataspace_id))',
      '(reserved_entrypoints.contains(&entrypoint) || route_conflict).then_some(index)',
      'ordinary work conflicts with an already-reserved autonomous lane slot',
      'let overflow = lane_session_overflow_indices(',
      'autonomous_lane_payloads.len(),',
      'self.limits.session_capacity.get(),',
      'ordinary lane routes exceed capacity after reserved autonomous work')),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::bind_locked_global_body_from_origin",
        (
            '!origin_matches || block.header().height().get() != self.context.height',
            'if crate::block::external_queue_plan_synced_entrypoint_index(block).is_some()',
            'retain_pending_certified_merge_entry_for_locked_carrier(',
            'retire_autonomous_payload_batch(&losing_pending)',
            'let canonical_recovery = (|| -> crate::kura::Result<bool> {',
            'Ok(canonical_v2_lane_payload_matches_kura(',
            'let canonical_recovery = match self.consensus_storage_read(canonical_recovery) {',
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
    ('crates/iroha_core/src/sumeragi/v2_candidate.rs',
     'method',
     'V2CandidateAssembler::snapshot_routable_candidates',
     ('let mut queue_plan_barrier = false;',
      'if queue_plan_barrier && !exact_height_lifecycle_transaction(context, &transaction)',
      'let queue_plan_synced =',
      'Ok(None) => {',
      'queue_plan_barrier = true;',
      'continue;',
      'if queue_plan_barrier',
      'exact_height_lifecycle_candidate(',
      'crate::torii_proxy::validate_queue_plan_binding_for_request(',
      'report.routable = report.routable.saturating_add(1)',
      'if queue_plan_synced',
      'report.work_deferred = report.work_deferred.saturating_add(1)',
      'queue_plan_barrier = true;',
      'continue;',
      'records.push(CandidateRecord {')),
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
    ('crates/iroha_core/src/sumeragi/v2_lane_work.rs',
     'method',
     '&mut V2LaneWorkAdapter::prepare',
     ('if context != &self.context',
      'match self.refresh_merge_candidates(view)',
      'let reserved_routes = self',
      'let reserved_entrypoints = self',
      'candidate.transaction().entrypoint().admission_intent()',
      '== iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced)',
      '.then_some(index)',
      'if !unavailable.is_empty()',
      'QueuePlanSynced work requires its globally admitted autonomous reservation',
      'let route_conflict = candidate.routing_plan().legs().iter().any(|leg| {',
      'reserved_routes.contains(&(leg.route.lane_id, leg.route.dataspace_id))',
      '(reserved_entrypoints.contains(&entrypoint) || route_conflict).then_some(index)',
      'if !unavailable.is_empty()',
      'ordinary work conflicts with an already-reserved autonomous lane slot',
      'let overflow = lane_session_overflow_indices(',
      'if !overflow.is_empty()',
      'ordinary lane routes exceed capacity after reserved autonomous work',
      'let lane_plan = prepare_v2_lane_payload_plan(')),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::bind_locked_global_body_from_origin",
        (
            "if !origin_matches || block.header().height().get() != self.context.height",
            "external_queue_plan_synced_entrypoint_index(block).is_some()",
            "return V2LaneIngressOutcome::Rejected;",
            "let canonical_recovery = (|| -> crate::kura::Result<bool> {",
            'let canonical_recovery = match self.consensus_storage_read(canonical_recovery) {',
            "retain_pending_certified_merge_entry_for_locked_carrier(",
            "retire_autonomous_payload_batch(&losing_pending)",
        ),
    ),
)

QUEUE_PLAN_AUTONOMOUS_ONLY_TEST_BINDINGS = (
    ('crates/iroha_core/src/sumeragi/v2_lane_work.rs',
     'candidate_provider_admits_ordinary_work_in_multiroute_world_and_excludes_queue_plan_synced',
     ('assert!(proposal_lookahead_enabled(',
      '(LaneId::SINGLE, DataSpaceId::UNIVERSAL)',
      '.expect("ordinary work cannot wait for an unavailable autonomous reservation")',
      'assert_eq!(prepared.lane_payload_ownerships.len(), 1)',
      'assert!(prepared.autonomous_lane_payloads.is_empty())',
      '.expect_err("QueuePlanSynced cannot bypass its autonomous ownership corridor")',
      'assert_eq!(unavailable.indices(), &BTreeSet::from([0]))')),
    ('crates/iroha_core/src/sumeragi/v2_lane_work_autonomous_ready_durability_tests.rs',
     'repeated_non_empty_retries_never_make_queue_plan_synced_work_ordinary_eligible',
     ('TransactionAdmissionIntent::QueuePlanSynced',
      '.expect_err("autonomous route must remain unavailable to ordinary execution")',
      'QueuePlanSynced work requires its globally admitted autonomous reservation')),
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
            ".contains_key(&fixture.stateless_cache_key)",
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
            "Self::durable_plan_claim_route_authority_in_view(state_view, &claim)",
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
            "IrohaNetwork::start_with_crypto_and_initial_authorities(",
        ),
    ),
)
QUEUE_PLAN_STARTUP_REPLAY_ORDERED_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::install_lane_reservation_journal",
        (
            "journal.consume_snapshot_replay_seal(replay_seal)?",
            "*store = candidate_store;",
            "self.lane_reservation_reconciliation_pending",
            ".store(true, Ordering::Release);",
            "Some(replay_receipt)",
            "*journal_guard = Some(journal);",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::bind_lane_reservation_startup_reconciliation_receipt",
        (
            "self.lane_reservation_startup_completion.lock().is_some()",
            "let observed = self.lane_reservation_reconciliation_snapshot()?;",
            "if observed != *expected_snapshot",
            "revalidate_queue_plan_startup_replay_receipt(",
            "if !self",
            ".lane_reservation_reconciliation_pending",
            ".load(Ordering::Acquire)",
            "return Err(LaneQueueReservationError::InvalidIdentity(",
            "Ok(Some(LaneReservationStartupReconciliationReceipt {",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::revalidate_lane_reservation_startup_reconciliation_receipt",
        (
            "self.lane_reservation_startup_completion.lock().is_some()",
            "|| !self.lane_reservation_startup_reconciliation_pending()",
            "receipt.initial_snapshot != *expected_snapshot",
            "return Ok(false);",
            "self.lane_reservation_reconciliation_snapshot()? != *expected_snapshot",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::revalidate_lane_reservation_startup_reconciliation_receipt_locked",
        (
            "self.lane_reservation_startup_completion.lock().is_some()",
            "|| !self.lane_reservation_startup_reconciliation_pending()",
            "receipt.initial_snapshot != *expected_snapshot",
            "self.lane_reservation_reconciliation_snapshot_locked()? != *expected_snapshot",
            "return Ok(false);",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::complete_lane_reservation_startup_reconciliation",
        (
            "self.lane_reservation_transition_lock.lock()",
            "self.push_remove_lock.lock()",
            "let reconciliation_pending = self",
            "self.lane_reservation_startup_completion.lock().is_some() || !reconciliation_pending",
            "return Err(LaneQueueReservationError::InvalidIdentity(",
            "let final_snapshot = self.lane_reservation_reconciliation_snapshot_locked()?;",
            "*self.lane_reservation_startup_completion.lock() =",
            "Some(CompletedLaneReservationStartupReconciliation {",
            "self.lane_reservation_reconciliation_pending",
            ".store(false, Ordering::Release);",
        ),
    ),
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
            "Self::durable_plan_claim_route_authority_in_view(state_view, &claim)",
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
            "IrohaNetwork::start_with_crypto_and_initial_authorities(",
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
        "queue_plan_journal_replay_retains_entrypoint_that_fails_stateless_revalidation",
        (
            'expect_err("wrong-network journal entrypoint must fail startup")',
            "failed canonical stateless validation",
            "assert!(!replay_queue.txs.contains_key(&hash));",
            "live_record_count()",
            "stateless failure must not append a tombstone or replacement",
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


# The semantic expectations are independent of refreshed item digests. In particular,
# adding a shipping fixture variant or returning before the checked transition cannot
# be legitimized by updating the seal of the changed implementation.
_DIRECT_RELEASE_PRODUCTION_ITEM_SHA256 = {
    'release_strictly_absent_lane_reservations_in_order': '19c28cbdf28b6750e25352e784c665c1b9ce20b581e9ea9d33762f77e2ab8471',
    'release_lane_reservations_in_order_inner': 'e9aef1a6a41dc0030554c1b850389367e577a1e6b04e71af6efb53e328a49f9d',
}

_DIRECT_RELEASE_PRODUCTION_SOURCE = {
    'release_strictly_absent_lane_reservations_in_order': r"""
    pub(crate) fn release_strictly_absent_lane_reservations_in_order(
        &self,
        keys: &[LaneQueueReservationKeyV1],
        authorizations: Vec<StrictAbsenceDirectReleaseAuthorization>,
    ) -> Result<usize, LaneQueueReservationError> {
        self.release_lane_reservations_in_order_inner(
            keys,
            LaneQueueDirectReleaseGate::StrictAbsence(authorizations),
        )
    }
""",
    'release_lane_reservations_in_order_inner': r"""
    fn release_lane_reservations_in_order_inner(
        &self,
        keys: &[LaneQueueReservationKeyV1],
        gate: LaneQueueDirectReleaseGate,
    ) -> Result<usize, LaneQueueReservationError> {
        let checked_direct_release = matches!(&gate, LaneQueueDirectReleaseGate::StrictAbsence(_));
        if self.transaction_selection_durability_faulted() {
            return Err(LaneQueueReservationError::DurabilityFault);
        }
        let mut entrypoint_hashes = BTreeSet::new();
        for key in keys {
            key.validate()
                .map_err(|reason| LaneQueueReservationError::InvalidIdentity(reason.to_owned()))?;
            if !entrypoint_hashes.insert(key.entrypoint_hash) {
                return Err(LaneQueueReservationError::InvalidIdentity(
                    "ordered lane reservation release contains a duplicate entrypoint".to_owned(),
                ));
            }
        }
        match &gate {
            LaneQueueDirectReleaseGate::StrictAbsence(authorizations) => {
                let mut authorized_groups = BTreeSet::new();
                let mut authorized_hashes = BTreeSet::new();
                for authorization in authorizations {
                    let (group, group_keys, projection) =
                        authorization.queue_group().ok_or_else(|| {
                            LaneQueueReservationError::InvalidIdentity(
                                "strict-absence direct-release authority is malformed".to_owned(),
                            )
                        })?;
                    if group_keys.is_empty()
                        || !authorized_groups.insert(group.identity)
                        || projection.before.queue.reservation_state
                            != IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE
                        || projection.after.queue.reservation_state
                            != IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED
                        || !projection.after.release.fifo_restored
                    {
                        return Err(LaneQueueReservationError::InvalidIdentity(
                            "strict-absence direct-release authority has a duplicate group or invalid terminal state"
                                .to_owned(),
                        ));
                    }
                    for key in group_keys {
                        if !authorized_hashes.insert(key.entrypoint_hash) {
                            return Err(LaneQueueReservationError::InvalidIdentity(
                                "strict-absence direct-release groups overlap one Queue owner"
                                    .to_owned(),
                            ));
                        }
                    }
                }
                if authorized_hashes != entrypoint_hashes {
                    return Err(LaneQueueReservationError::InvalidIdentity(
                        "strict-absence direct-release authorities differ from the exact global FIFO set"
                            .to_owned(),
                    ));
                }
            }
            #[cfg(test)]
            LaneQueueDirectReleaseGate::Fixture => {}
        }
        let _reservation_transition_guard = self.lane_reservation_transition_lock.lock();
        let queue_guard = self.push_remove_lock.lock();
        if self.transaction_selection_durability_faulted() {
            return Err(LaneQueueReservationError::DurabilityFault);
        }
        match &gate {
            LaneQueueDirectReleaseGate::StrictAbsence(authorizations) => {
                for authorization in authorizations {
                    let (group, group_keys, _) = authorization.queue_group().ok_or_else(|| {
                        LaneQueueReservationError::InvalidIdentity(
                            "strict-absence direct-release authority changed under the Queue lock"
                                .to_owned(),
                        )
                    })?;
                    self.revalidate_complete_live_pre_kura_group_locked(group, group_keys)?;
                }
            }
            #[cfg(test)]
            LaneQueueDirectReleaseGate::Fixture => {}
        }
        let store = self.lane_reservations.lock();
        for key in keys {
            store.ensure_no_conflict(key)?;
            store.ensure_not_release_prepared(key)?;
        }
        let records = keys
            .iter()
            .filter_map(|key| {
                store
                    .live_by_entrypoint
                    .get(&key.entrypoint_hash)
                    .cloned()
                    .map(|record| (*key, record))
            })
            .collect::<Vec<_>>();
        match &gate {
            LaneQueueDirectReleaseGate::StrictAbsence(_) => {
                if records.len() != keys.len() {
                    return Err(LaneQueueReservationError::InvalidIdentity(
                        "strict-absence direct release lost an exact live reservation before its sink"
                            .to_owned(),
                    ));
                }
            }
            #[cfg(test)]
            LaneQueueDirectReleaseGate::Fixture => {}
        }
        for (_, record) in &records {
            self.validate_live_reservation_against_queue(record)?;
        }
        if records
            .windows(2)
            .any(|records| records[0].1.fifo_order.ordinal >= records[1].1.fifo_order.ordinal)
        {
            return Err(LaneQueueReservationError::InvalidIdentity(
                "ordered lane reservation release does not follow original global FIFO order"
                    .to_owned(),
            ));
        }
        let released_records = records
            .iter()
            .map(|(_, record)| record.clone())
            .collect::<Vec<_>>();
        // Preflight capacity and stable ordinals before the durable append. Unrelated hashes may
        // continue to enter or leave FIFO while fsync runs; publication therefore rebuilds from a
        // fresh locked snapshot below instead of replacing FIFO with this stale observation.
        self.fifo_with_released_reservations_locked(&released_records)?;
        let transition = self
            .begin_durability_transition_locked(records.iter().map(|(key, _)| key.entrypoint_hash))
            .map_err(|hash| LaneQueueReservationError::Conflict { hash })?;
        let release_keys = records.iter().map(|(key, _)| *key).collect();
        drop(store);
        drop(queue_guard);
        self.apply_lane_reservation_journal(move |journal| {
            match gate {
                LaneQueueDirectReleaseGate::StrictAbsence(authorizations) => {
                    for authorization in authorizations {
                        let projection = authorization.consume_for_queue().ok_or_else(|| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "strict-absence direct-release authority changed before append",
                            )
                        })?;
                        let terminal =
                            production_in_flight_first_release_terminal_owner(projection.after)
                                .ok_or_else(|| {
                                    std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "strict-absence direct release has no terminal owner",
                                    )
                                })?;
                        if !terminal.ordinary_fifo_owner
                            || terminal.canonical_wsv_owner
                            || terminal.commit_terminal
                            || !terminal.release_terminal
                        {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "strict-absence direct release is not FIFO-only terminal ownership",
                            ));
                        }
                    }
                }
                #[cfg(test)]
                LaneQueueDirectReleaseGate::Fixture => {}
            }
            journal.release_batch(release_keys)
        })?;
        let queue_guard = self.push_remove_lock.lock();
        let mut store = self.lane_reservations.lock();
        let restored_fifo = match self.fifo_with_released_reservations_locked(&released_records) {
            Ok(fifo) => fifo,
            Err(error) => {
                let error = std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "cannot publish durably released lane reservations against the current FIFO: {error}"
                    ),
                );
                self.latch_lane_reservation_post_journal_publication_fault_locked(&error);
                drop(store);
                drop(transition);
                drop(queue_guard);
                self.publish_latched_lane_reservation_durability_fault(None);
                return Err(LaneQueueReservationError::Journal(error));
            }
        };
        for (key, _) in &records {
            store.live_by_entrypoint.remove(&key.entrypoint_hash);
            if checked_direct_release {
                self.durable_plan_claims
                    .get_mut(&key.entrypoint_hash)
                    .expect("the direct-release transition retains its validated admission claim")
                    .local_custody = QueuePlanLocalCustody::Available;
            }
        }
        self.replace_fifo_locked(&restored_fifo);
        self.reconcile_missing_reservation_payloads_locked(&mut store);
        drop(store);
        drop(transition);
        drop(queue_guard);
        let publish_fault = self.compact_lane_reservations_off_lock();
        if publish_fault {
            self.publish_latched_lane_reservation_durability_fault(None);
        }
        Ok(records.len())
    }
""",
}

_DIRECT_RELEASE_GATE_SOURCE = """
enum LaneQueueDirectReleaseGate {
    StrictAbsence(Vec<StrictAbsenceDirectReleaseAuthorization>),
    #[cfg(test)]
    Fixture,
}
"""


def validate_direct_release_authority_contract(
    root: Path, errors: list[str], rust_binding_item: Any
) -> None:
    """Require one shipping authority path and reject all raw-key test escapes."""
    import check_sumeragi_v2_proof_ledger as ledger

    queue_path = root / "crates/iroha_core/src/queue.rs"
    journal_path = root / "crates/iroha_core/src/queue/reservation_journal.rs"
    sources = {}
    for path in (queue_path, journal_path):
        if path.is_symlink() or not path.is_file():
            errors.append(f"{path}: direct-release authority requires a regular source owner")
            return
        sources[path] = path.read_text(encoding="utf-8")
    for path, owner, name in (
        (queue_path, "Queue", "release_lane_reservation"),
        (queue_path, "Queue", "release_lane_reservations_in_order"),
        (journal_path, "LaneQueueReservationJournal", "release"),
    ):
        item = ledger._require_qualified_rust_item(
            path, sources[path], owner, name, errors,
            "raw-key direct release must remain a test-only fixture",
            expected_attributes=("#[cfg(test)]",),
        )
        if item is not None:
            visibility = "pub(super)" if path == journal_path else "pub(crate)"
            expected = ledger.rust_code_tokens(f"{visibility} fn {name}")
            if ledger.rust_code_tokens(item.source)[:len(expected)] != expected:
                errors.append(f"{path}: raw-key direct release must retain crate-local test visibility")
    for name, expected_source in _DIRECT_RELEASE_PRODUCTION_SOURCE.items():
        item = ledger._require_qualified_rust_item(
            queue_path, sources[queue_path], "Queue", name, errors,
            "direct-release authority requires its shipping Queue owner",
        )
        ledger._require_rust_item_token_sha256(
            queue_path, item, _DIRECT_RELEASE_PRODUCTION_ITEM_SHA256[name],
            "direct-release authority source seal", errors,
        )
        ledger._require_exact_rust_tokens(
            queue_path, item, expected_source,
            "direct-release authority must be mandatory through the exact journal sink", errors,
        )
    gate = rust_binding_item(
        root, "crates/iroha_core/src/queue.rs", "enum", "LaneQueueDirectReleaseGate",
        "direct-release authority gate", errors,
    )
    if gate is not None and ledger.rust_code_tokens(gate) != ledger.rust_code_tokens(_DIRECT_RELEASE_GATE_SOURCE):
        errors.append(f"{queue_path}: direct-release authority gate must have only StrictAbsence in shipping builds")


def validate_current_queue_plan_selection(items: dict, errors: list[str]) -> None:
    """Bind unconditional signed-intent exclusion and exact request delegation."""
    symbol = "&mut V2LaneWorkAdapter::prepare"
    item = items.get(("crates/iroha_core/src/sumeragi/v2_lane_work.rs", "method", symbol))
    if item is not None:
        exact = """let unavailable = candidates.iter().copied().enumerate()
            .filter_map(|(index, candidate)| {
                (candidate.transaction().entrypoint().admission_intent()
                    == iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced)
                    .then_some(index)
            }).collect::<BTreeSet<_>>();
            if !unavailable.is_empty() {
                return Err(CandidateWorkUnavailable::new(unavailable,
                    "QueuePlanSynced work requires its globally admitted autonomous reservation",
                ).into());
            }"""
        conflict = """let unavailable = candidates.iter().copied().enumerate()
            .filter_map(|(index, candidate)| {
                let entrypoint = Hash::from(candidate.entrypoint_hash());
                let route_conflict = candidate.routing_plan().legs().iter().any(|leg| {
                    reserved_routes.contains(&(leg.route.lane_id, leg.route.dataspace_id))
                });
                (reserved_entrypoints.contains(&entrypoint) || route_conflict).then_some(index)
            }).collect::<BTreeSet<_>>();
            if !unavailable.is_empty() {
                return Err(CandidateWorkUnavailable::new(unavailable,
                    "ordinary work conflicts with an already-reserved autonomous lane slot",
                ).into());
            }"""
        for obligation in (exact, conflict):
            if _code(obligation) not in _code(item):
                errors.append(f"{symbol}: current QueuePlan intent or reserved-slot exclusion changed")
    symbol = "V2CandidateAssembler::snapshot_routable_candidates"
    item = items.get(("crates/iroha_core/src/sumeragi/v2_candidate.rs", "method", symbol))
    if item is not None:
        call = """crate::torii_proxy::validate_queue_plan_binding_for_request(
            &binding, state.network_id_ref(), transaction.entrypoint(), &routing_plan,
        )"""
        if _code(call) not in _code(item):
            errors.append(f"{symbol}: exact QueuePlan request authority delegation changed")

    request = items.get(("crates/iroha_core/src/torii_proxy.rs", "fn", "validate_queue_plan_binding_for_request"))
    if request is not None:
        guards = (
            """if binding.network_id_digest != queue_plan_admission_network_id_digest(network_id) {
                return Err("QueuePlan admission binding belongs to another network".to_owned());
            }""",
            """if binding.request_id != queue_plan_synced_request_id(network_id, transaction.hash()) {
                return Err("QueuePlan admission binding has a noncanonical semantic request identity".to_owned());
            }
            validate_queue_plan_binding_for_transaction_and_plan(binding, transaction, routing_plan)""",
        )
        if any(_code(guard) not in _code(request) for guard in guards):
            errors.append("QueuePlan binding network/request rejection or exact delegation changed")
    claim = items.get(("crates/iroha_core/src/torii_proxy.rs", "fn", "validate_queue_plan_binding_for_transaction_and_plan"))
    if claim is not None:
        exact = """let exact_digest = crate::queue::queue_plan_journal_record_claim_digest(
            transaction.clone(), routing_plan.clone(), binding.admission_context.clone(),
            binding.enqueue_timestamp_ms, Some(binding.global_admission_identity()),
        ).map_err(|error| format!("QueuePlan journal claim cannot be encoded: {error}"))?;
        if exact_digest != binding.journal_record_digest {
            return Err("QueuePlan admission binding does not cover the exact journal record".to_owned());
        } Ok(())"""
        if _code(exact) not in _code(claim):
            errors.append("QueuePlan complete journal claim or digest rejection changed")


# Retained authority is custody only; ordinary selectors remain closed.
QUEUE_PLAN_RETAINED_ROUTE_BINDINGS = (('crates/iroha_core/src/state.rs',
  'method',
  'State::queue_plan_pending_route_authority_in_view',
  ('match Self::queue_plan_binding_application_evidence_in_view(state, binding)? {\n'
   '            QueuePlanBindingApplicationEvidence::Absent\n'
   '            | QueuePlanBindingApplicationEvidence::AppliedDirect\n'
   '            | QueuePlanBindingApplicationEvidence::AppliedViaSignedAlias => return Ok(None),\n'
   '            QueuePlanBindingApplicationEvidence::PendingStale => {\n'
   '                return Err("retained QueuePlan input has a stale route '
   'incarnation".to_owned());\n'
   '            }\n'
   '            QueuePlanBindingApplicationEvidence::Pending => {}\n'
   '        }',
   'let next_height = height\n            .checked_add(1)',
   'let predecessor = if context.authority_height == 0 {\n'
   '            None\n'
   '        } else {\n'
   '            let index = usize::try_from(context.authority_height - 1)',
   'let record = Self::decode_exact_queue_plan_admission_registry_record(\n'
   '            &registry_key,\n'
   '            state\n'
   '                .world()\n'
   '                .smart_contract_state()\n'
   '                .get(&registry_key)',
   'if predecessor != context.predecessor_block_hash\n'
   '            || record.claim != binding.registry_value()\n'
   '            || record.priority.carrier_height < context.proposal_height\n'
   '            || record.priority.carrier_height > height\n'
   '        {\n'
   '            return Err(',
   'for bound in &context.route_incarnations {',
   '.find(|lane| lane.id == route.lane_id && lane.dataspace_id == route.dataspace_id)',
   'let drain = decode_autoscale_lane_drain_state(lane).map_err(str::to_owned)?;\n'
   '            if let Some(drain) = drain\n'
   '                && next_height > drain.intent.close_global_height\n'
   '            {',
   'let pin = decode_autoscale_lane_committee(lane)\n'
   '                    .map_err(str::to_owned)?\n'
   '                    .ok_or_else(|| "retained QueuePlan drain has no immutable '
   'pin".to_owned())?;\n'
   '                validate_autoscale_lane_committee_pops(&pin).map_err(str::to_owned)?;',
   'if !lane.claims_autoscale_managed()\n'
   '                    || close > height\n'
   '                    || record.priority.carrier_height > close\n'
   '                    || drain.commitment.is_some()\n'
   '                    || !autoscale_lane_drain_state_matches_context(\n'
   '                        lane,\n'
   '                        &drain,\n'
   '                        state.network_id(),\n'
   '                        bound.lane_incarnation,\n'
   '                    )\n'
   '                    || state.lane_incarnation_at_height(route.lane_id, close)\n'
   '                        != Some(bound.lane_incarnation)\n'
   '                    || pin.validator_set != bound.validator_set\n'
   '                {\n'
   '                    return Err(',
   'authority = QueuePlanPendingRouteAuthority::Draining;\n'
   '            } else if state.lane_incarnation_at_height(route.lane_id, next_height)\n'
   '                != Some(bound.lane_incarnation)\n'
   '            {\n'
   '                return Err(',
   'Ok(Some(authority))')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::durable_plan_claim_route_authority_in_view',
  ('let active = resolve_routing_plan_for_queue_admission(\n'
   '            claim.routing_plan.clone(),\n'
   '            state_view.nexus(),\n'
   '            u64::try_from(state_view.height()).unwrap_or(u64::MAX),\n'
   '        );\n'
   '        if active.as_ref() == Ok(&claim.routing_plan)\n'
   '            && Self::durable_plan_claim_context_revalidates_in_view(\n'
   '                state_view,\n'
   '                &claim.routing_plan,\n'
   '                &claim.admission_context,\n'
   '            )\n'
   '        {\n'
   '            return Ok(QueuePlanPendingRouteAuthority::Active);\n'
   '        }',
   'if claim.global_admission_identity.is_some() {\n'
   '            let binding = claim\n'
   '                .global_admission_binding()\n'
   '                .map_err(|_| RoutingResolveError::StaleRoutingPlan)?;\n'
   '            if let Some(authority) =\n'
   '                State::queue_plan_pending_route_authority_in_view(state_view, &binding)\n'
   '                    .map_err(|_| RoutingResolveError::StaleRoutingPlan)?\n'
   '            {\n'
   '                return Ok(authority);\n'
   '            }\n'
   '        }\n'
   '        Err(active\n'
   '            .err()\n'
   '            .unwrap_or(RoutingResolveError::StaleRoutingPlan))')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::revalidated_durable_plan_claim_retry_locked',
  ('&& &existing.routing_plan == routing_plan\n'
   '            && &existing.admission_context == expected_admission_context',
   'if Self::durable_plan_claim_route_authority_in_view(state_view, &existing).is_err() {\n'
   '            return Err(')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::has_revalidatable_durable_plan_claim_with_state',
  ('let _lifecycle_guard = state.lock_lane_lifecycle_work_admission();\n'
   '        let state_view = state.view();',
   'if tx.has_committed_replay_identity(&state_view) {\n'
   '            return false;\n'
   '        }\n'
   '        let _queue_guard = self.push_remove_lock.lock();',
   'let current_plan = if immutable_owner {\n'
   '            self.durable_plan_claims\n'
   '                .get(&tx_hash)\n'
   '                .ok_or(RoutingResolveError::StaleRoutingPlan)\n'
   '                .and_then(|claim| {\n'
   '                    Self::durable_plan_claim_route_authority_in_view(&state_view, &claim)\n'
   '                })\n'
   '                .map(|_| routing_plan.clone())\n'
   '        } else {\n'
   '            self.resolve_precomputed_routing_plan_with_view(tx, &state_view, '
   'routing_plan.clone())\n'
   '        };',
   'match self.revalidated_durable_plan_claim_retry_locked(\n'
   '            tx,\n'
   '            &state_view,\n'
   '            &current_plan,\n'
   '            expected_admission_context,\n'
   '        ) {\n'
   '            Ok(Some(_)) => true,\n'
   '            Ok(None) => false,')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::immutable_queued_routing_plan_in_view',
  ('let exact_claim = claim.entrypoint_hash == tx.as_accepted().hash_as_entrypoint()\n'
   '                && claim.signed_transaction_hash\n'
   '                    == '
   'crate::tx::exact_signed_transaction_hash(tx.as_accepted().entrypoint())\n'
   '                && claim.routing_plan == plan;\n'
   '            if !exact_claim {\n'
   '                return Err(RoutingResolveError::StaleRoutingPlan);\n'
   '            }\n'
   '            Self::durable_plan_claim_route_authority_in_view(state_view, &claim)?',
   'Ok((plan, authority))')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::immutable_queued_routing_plan_with_view',
  ('self.immutable_queued_routing_plan_if_available_in_view(',
   'retained.and_then(|(plan, authority)| {\n'
   '                (authority == QueuePlanPendingRouteAuthority::Active).then_some(plan)\n'
   '            })')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::reserve_transactions_for_lane_bounded',
  ('let routing_plan = match self.immutable_queued_routing_plan_in_view(',
   'Ok((_, QueuePlanPendingRouteAuthority::Draining)) => continue,\n'
   '                Ok((routing_plan, QueuePlanPendingRouteAuthority::Active)) => routing_plan,')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::pop_from_queue',
  ('let routing_plan = match self.immutable_queued_routing_plan_with_view(',
   'Ok(None) => {\n'
   '                    let queue_guard = self.push_remove_lock.lock();\n'
   '                    let restore_error = self.restore_popped_hash_locked(hash);\n'
   '                    drop(queue_guard);')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::bounded_pending_snapshot',
  ('if !open {\n'
   '                            match State::queue_plan_pending_route_authority_in_view(\n'
   '                                state_view, &binding,\n'
   '                            ) {\n'
   '                                Ok(Some(QueuePlanPendingRouteAuthority::Draining)) => {\n'
   '                                    blocked_by_fifo_predecessor = true;\n'
   '                                    return None;\n'
   '                                }',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::push_with_lane_internal_with_state_and_routing',
  ('let canonical_pending_handoff = if let Some(binding) = expected_admission_binding {',
   'Ok(Some(canonical_binding)) if canonical_binding == *binding => {',
   'State::queue_plan_pending_route_authority_in_view(&state_view, binding)\n'
   '                            .map_err(|reason| Failure {\n'
   '                                tx: tx.clone().into(),\n'
   '                                err: Error::UnresolvedRoute { reason },\n'
   '                            })?\n'
   '                            .is_some()')))


QUEUE_PLAN_RETAINED_ROUTE_BINDINGS += (('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::revalidate_pending_transactions',
  ('let routing_plan = match self.immutable_queued_routing_plan_in_view(\n'
   '                hash,\n'
   '                tx.as_ref(),\n'
   '                state_view,\n'
   '                &routing_nexus,\n'
   '                block_height,\n'
   '            ) {\n'
   '                Ok((plan, _)) => plan,',
   'if matches!(err, RoutingResolveError::StaleRoutingPlan)\n'
   '                        || routing_generation_unchanged\n'
   '                        || self.durable_plan_claims.contains_key(&hash)\n'
   '                    {\n'
   '                        corrupt_ownership.push((hash, err));')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::durable_plan_admission_claim_with_state',
  ('&& Self::durable_plan_claim_route_authority_in_view(&state_view, &claim).is_ok();\n'
   '        if !exact_owner {',
   'context: claim.admission_context,\n'
   '            global_admission_identity: claim.global_admission_identity,\n'
   '            routing_plan: claim.routing_plan,\n'
   '            entrypoint_hash: claim.entrypoint_hash,\n'
   '            signed_transaction_hash: claim.signed_transaction_hash,\n'
   '            enqueue_timestamp_ms: claim.enqueue_timestamp_ms,\n'
   '            journal_record_digest: claim.journal_record_digest,\n'
   '        }))')))


# Native opening consumes the same retained authority; its staged carrier cut stays exact.
QUEUE_PLAN_RETAINED_ROUTE_BINDINGS += (('crates/iroha_core/src/state/lane_consensus_authority.rs',
  'fn',
  'resolve_open_lane_authority',
  ('if height == 0\n'
   '        || height != state._curr_block.height().get()\n'
   '        || !state\n'
   '            .nexus\n'
   '            .lane_catalog\n'
   '            .lanes()\n'
   '            .iter()\n'
   '            .any(|current| current == lane)\n'
   '        || state.lane_incarnations.get(&lane.id).copied() != Some(incarnation)\n'
   '        || lane_incarnation_is_zero(incarnation)\n'
   '    {\n'
   '        return Err(',
   'let activation = state\n'
   '        .lane_incarnation_activation_heights\n'
   '        .get(&lane.id)\n'
   '        .and_then(|height| height.checked_add(1))',
   'if height < activation {\n'
   '        return Err("lane opening authority precedes incarnation activation".to_owned());\n'
   '    }',
   'let drain = decode_autoscale_lane_drain_state(lane).map_err(str::to_owned)?;\n'
   '    let (committee, pops) = if let Some(drain) = drain\n'
   '        && height > drain.intent.close_global_height\n'
   '    {',
   'let committed_height = u64::try_from(state.height())\n'
   '            .map_err(|_| "lane opening committed height exceeds u64".to_owned())?;\n'
   '        if !lane.claims_autoscale_managed()\n'
   '            || drain.intent.close_global_height > committed_height\n'
   '            || drain.intent.close_global_height < activation\n'
   '            || drain.commitment.is_some()\n'
   '            || !autoscale_lane_drain_state_matches_context(\n'
   '                lane,\n'
   '                &drain,\n'
   '                &state.network_id,\n'
   '                incarnation,\n'
   '            )\n'
   '            || !nexus_autoscale_lane_active_for_authority(\n'
   '                lane,\n'
   '                &state.nexus,\n'
   '                drain.intent.close_global_height,\n'
   '            )\n'
   '        {\n'
   '            return Err(',
   'let route = QueuePlanPendingObligationRouteV1 {\n'
   '            version: QUEUE_PLAN_PENDING_OBLIGATION_VERSION_V1,\n'
   '            lane_id: lane.id,\n'
   '            dataspace_id: lane.dataspace_id,\n'
   '            lane_incarnation: incarnation,\n'
   '        };\n'
   '        let members = State::queue_plan_pending_route_members_from_storage(\n'
   '            state.world.smart_contract_state(),\n'
   '            route,\n'
   '        )\n'
   '        .map_err(|error| error.to_string())?;\n'
   '        if members.is_empty() {\n'
   '            return Err("closed lane opening has no admitted pending work".to_owned());\n'
   '        }\n'
   '        for (_, member) in members {',
   'let obligation =\n'
   '                State::decode_exact_queue_plan_pending_obligation_marker(&key, payload)\n'
   '                    .map_err(|error| error.to_string())?;\n'
   '            if State::queue_plan_pending_route_authority_in_view(state, &obligation.binding)?\n'
   '                != Some(QueuePlanPendingRouteAuthority::Draining)\n'
   '            {\n'
   '                return Err(\n'
   '                    "closed lane opening requires exact unresolved pre-close '
   'admissions".to_owned(),\n'
   '                );\n'
   '            }',
   'let pin = decode_autoscale_lane_committee(lane)\n'
   '            .map_err(str::to_owned)?\n'
   '            .ok_or_else(|| "closed lane opening has no immutable committee pin".to_owned())?;\n'
   '        validate_autoscale_lane_committee_pops(&pin).map_err(str::to_owned)?;\n'
   '        (pin.validator_set, pin.validator_pops)\n'
   '    } else {',
   'iroha_data_model::block::consensus_v2::finality::verify_validator_power_roster_pops(\n'
   '        &roster, &pops,\n'
   '    )\n'
   '    .map_err(|error| error.to_string())?;\n'
   '    Ok((committee, pops))')),)


def _merge_retained_queue_plan_bindings(existing: tuple, retained: tuple) -> tuple:
    """Add exact retained-owner declarations without duplicating existing owners."""
    result = list(existing)
    for path, kind, symbol, tokens in retained:
        matches = [i for i, row in enumerate(result) if row[:3] == (path, kind, symbol)]
        if matches:
            assert len(matches) == 1
            i = matches[0]
            result[i] = (path, kind, symbol, result[i][3] + tokens)
        else:
            result.append((path, kind, symbol, tokens))
    return tuple(result)


QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS = _merge_retained_queue_plan_bindings(
    QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS, QUEUE_PLAN_RETAINED_ROUTE_BINDINGS
)


def validate_retained_queue_plan_route_authority(items: dict, errors: list[str]) -> None:
    """Bind executable close/pin/all-leg predicates and the non-execution projection."""
    for path, kind, symbol, obligations in QUEUE_PLAN_RETAINED_ROUTE_BINDINGS:
        item = items.get((path, kind, symbol))
        if item is None:
            errors.append(f"{symbol}: missing retained QueuePlan route authority owner")
            continue
        code = _code(item)
        for obligation in obligations:
            if _code(obligation) not in code:
                errors.append(f"{symbol}: retained QueuePlan route authority relation changed: {obligation!r}")


# Exact canonical retry: existing replicated custody does not reopen admission.
QUEUE_PLAN_CANONICAL_RETRY_BINDINGS = (('crates/iroha_core/src/state.rs',
  'method',
  'State::canonical_queue_plan_admitted_input',
  ('let observe = || -> Result<',
   'let view = self.view();',
   'Self::queue_plan_admission_registry_value_in_view(&view, entrypoint_hash)?;',
   'Self::decode_exact_queue_plan_admission_registry_record(&key, payload)',
   'Self::queue_plan_registry_owner_application_state_in_view(\n'
   '                &view,\n'
   '                network_id_digest,\n'
   '                entrypoint_hash,\n'
   '                record.claim.binding_hash,\n'
   '            )',
   'if application == QueuePlanAdmissionApplicationState::PendingStale {',
   'view.block_hashes().get(index).copied().ok_or_else(',
   'Ok(Some((record, carrier_hash)))',
   'let Some((record, carrier_hash)) = observe()? else {\n'
   '                return Ok(None);\n'
   '            };',
   '.read_first_admission_carrier(height, carrier_hash)',
   'if read.finality.height_context.network_id != self.network_id\n'
   '                    || read.finality.height != record.priority.carrier_height\n'
   '                    || read.finality.height_context.height != record.priority.carrier_height',
   'let body = read.body.ok_or_else(',
   'let index = usize::try_from(record.priority.admission_index)',
   '.and_then(|context| context.queue_plan_admissions.get(index))',
   'crate::torii_proxy::decode_and_validate_lane_admitted_input_v1(\n'
   '                    &self.network_id,\n'
   '                    bytes,\n'
   '                )?',
   'if input.entrypoint().hash() != entrypoint_hash\n'
   '                    || input.certificate().registry_key != registry_key\n'
   '                    || input.certificate().registry_value != record.claim',
   '.proposal_height\n                        > record.priority.carrier_height',
   'if observe()?.as_ref() != Some(&(record, carrier_hash)) {\n                return Err(',
   'result.map(Some)',
   'let limits = Self::canonical_queue_plan_input_decode_limits()',
   'norito::with_decode_limits_scope(limits, || {')),
 ('crates/iroha_core/src/kura/lane_admission_source.rs',
  'method',
  'Kura::read_first_admission_carrier',
  ('let _prune = self.prune_lock.lock();',
   'self.ensure_prune_recovery_not_required()?;',
   'let _canonical = self.canonical_chain_lock.lock();',
   'self.read_first_admission_carrier_under_prune_and_canonical_guards(height, expected_hash)')),
 ('crates/iroha_core/src/kura/lane_admission_source.rs',
  'method',
  'Kura::read_first_admission_carrier_under_prune_and_canonical_guards',
  ('self.ensure_prune_recovery_not_required()?;',
   'self.ensure_canonical_storage_not_poisoned()?;',
   '.ok_or(Error::MissingV2FinalityArtifact { height: height_u64 })?',
   'if header.hash() != expected_hash || finality.block_hash != expected_hash {',
   'let body = self.read_block_body_under_prune_and_canonical_guards(height)?;',
   'if let Some(body) = &body\n'
   '            && (body.header() != header\n'
   '                || body.canonical_proposal_wire_hash()? != finality.subject.payload_hash)',
   'Ok(FinalizedAdmissionCarrierReadV1 { finality, body })',
   '.decode_v2_finality_record_at(&path, &directory)?',
   'Self::validate_v2_finality_record_at(&path, height_u64, expected_hash, &record)?;',
   '.retained_block_record_at_without_live_body(&blocks_dir, height_u64, expected_hash)?',
   'if header != record.block_header {',
   'Self::validate_v2_finality_wire_bindings(',
   'self.verify_v2_finality_artifact_at(&path, &directory, &record.artifact, &read_identity)?;',
   'drop(read_identity);')),
 ('crates/iroha_torii/src/lib.rs',
  'fn',
  'canonical_queue_plan_submission_response',
  ('authenticated: &AuthenticatedQueuePlanRetry',
   '.queue_plan_admission_registry_entrypoint_present(entrypoint_hash)',
   'Ok(false) => None,',
   'Ok(true) => Some(transaction_submission_receipt_response(',
   'Err(error) => Some(queue_plan_admission_registry_conflict_response(',
   'let entrypoint_hash = authenticated.entrypoint_hash();',
   'Some(authenticated.signed_transaction_hash())')),
 ('crates/iroha_torii/src/lib.rs',
  'fn',
  'canonical_queue_plan_synced_response',
  ('.queue_plan_admission_binding_registry_match(binding)',
   'Ok(QueuePlanAdmissionRegistryMatch::Absent) => return None,',
   'Ok(QueuePlanAdmissionRegistryMatch::Exact) => {}',
   'Ok(QueuePlanAdmissionRegistryMatch::Conflict) => {\n'
   '            return Some(queue_plan_admission_registry_conflict_response(',
   'Err(error) => {\n            return Some(queue_plan_admission_registry_conflict_response(',
   'Ok(Some(input)) if &input.input().certificate.binding == binding => input,',
   'Ok(_) => {\n            return Some(queue_plan_outcome_unknown_response(',
   'Err(error) => {\n            return Some(queue_plan_outcome_unknown_response(',
   'utils::NoritoBody(input.into_input().certificate)',
   '.canonical_queue_plan_admitted_input(binding.entrypoint_hash)',
   'let reservation = match proxy_memory',
   '.unwrap_or_else(|| acquire_torii_proxy_memory(app))',
   'runtime.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread',
   'tokio::task::block_in_place(|| {',
   'Some(hold_torii_proxy_memory_in_response_body(\n'
   '        response,\n'
   '        reservation,\n'
   '    ))',
   'read_deadline: tokio::time::Instant',
   'if tokio::time::Instant::now() >= read_deadline {\n'
   '        return Some(queue_plan_outcome_unknown_response(',
   'authenticated: &AuthenticatedQueuePlanRetry',
   'if authenticated.entrypoint_hash() != binding.entrypoint_hash\n'
   '        || Some(authenticated.signed_transaction_hash()) != binding.signed_transaction_hash')),
 ('crates/iroha_torii/src/lib.rs',
  'fn',
  'submit_signed_transaction_for_ingress_queue_plan_certified',
  ('let prepared = prepare_fresh_transaction_ingress(&app, accepted_tx)?;',
   'submit_prepared_transaction_ingress(',
   'routing::accept_decoded_signed_transaction_for_ingress(',
   'run_transaction_ingress_compute_job(',
   'AuthenticatedQueuePlanRetry::from_signed(',
   'canonical_queue_plan_submission_response(',
   'return Ok(PreparedTransactionIngress::Canonical(response));',
   'PreparedTransactionIngress::Canonical(response) => return Ok(response),',
   'PreparedTransactionIngress::Fresh(accepted_tx) => accepted_tx')),
 ('crates/iroha_torii/src/lib.rs',
  'fn',
  'handler_post_transaction_entrypoint',
  ('routing::accept_transaction_for_ingress(state, transaction, &telemetry)',
   'let prepared = prepare_fresh_transaction_ingress(&app, accepted_tx)?;',
   'submit_prepared_transaction_ingress(',
   'run_transaction_ingress_compute_job(',
   'AuthenticatedQueuePlanRetry::from_entrypoint(',
   'canonical_queue_plan_submission_response(',
   'return Ok(PreparedTransactionIngress::Canonical(response));',
   'PreparedTransactionIngress::Canonical(response) => return Ok(response),',
   'PreparedTransactionIngress::Fresh(accepted_tx) => accepted_tx')),
 ('crates/iroha_torii/src/lib.rs',
  'struct',
  'PreparedFreshTransactionIngress',
  ("transaction: iroha_core::tx::AcceptedTransaction<'static>",
   'routing_plan: RoutingPlan',
   'durable_retry_claim: Option<queue::QueuePlanDurableAdmissionV1>')),
 ('crates/iroha_torii/src/lib.rs',
  'fn',
  'prepare_fresh_transaction_ingress',
  ("transaction: iroha_core::tx::AcceptedTransaction<'static>",
   '.durable_plan_admission_claim_with_state(&transaction, app.state.as_ref())',
   'if !durable_retry_claim\n'
   '        .as_ref()\n'
   '        .is_some_and(|claim| claim.global_admission_identity.is_some())\n'
   '    {\n'
   '        routing::reject_ingress_if_queue_capacity_saturated(\n'
   '            app.queue.as_ref(),\n'
   '            app.state.as_ref(),\n'
   '            1,\n'
   '        )?;\n'
   '    }',
   'let routing_plan = if let Some(claim) = &durable_retry_claim {\n'
   '        claim.routing_plan.clone()\n'
   '    } else {',
   '.route_plan_with_state(&transaction, app.state.as_ref())',
   'Ok(PreparedFreshTransactionIngress {\n'
   '        transaction,\n'
   '        routing_plan,\n'
   '        durable_retry_claim,\n'
   '    })')),
 ('crates/iroha_torii/src/lib.rs',
  'fn',
  'submit_prepared_transaction_ingress',
  ('prepared: PreparedFreshTransactionIngress',
   'let PreparedFreshTransactionIngress {\n'
   '        transaction,\n'
   '        routing_plan,\n'
   '        durable_retry_claim,\n'
   '    } = prepared;',
   'if let Some(authenticated) =\n'
   '            AuthenticatedQueuePlanRetry::from_accepted(app.state.network_id_ref(), &transaction)?\n'
   '            && let Some(response) = canonical_queue_plan_submission_response(\n'
   '                app.as_ref(),\n'
   '                &authenticated,\n'
   '                minimal_response,\n'
   '                format,\n'
   '            )\n'
   '        {\n'
   '            return Ok(response);\n'
   '        }',
   'let durable_retry_claim = app\n'
   '            .queue\n'
   '            .durable_plan_admission_claim_with_state(&transaction, app.state.as_ref())\n'
   '            .map_err(|error| routing_resolve_error_to_torii_error(app, error))?\n'
   '            .or(durable_retry_claim);',
   'let routing_plan = durable_retry_claim\n'
   '            .as_ref()\n'
   '            .map_or(routing_plan, |claim| claim.routing_plan.clone());',
   '.is_some_and(|claim| claim.global_admission_identity.is_some())',
   'let reservation = if already_durably_admitted {\n'
   '            None\n'
   '        } else {',
   'reserve_verified_transaction_authority(\n'
   '                    &app.tx_rate_limiter,\n'
   '                    transaction.authority_opt(),\n'
   '                )',
   'let response = execute_torii_transaction_via_proxy(\n'
   '            app,\n'
   '            transaction,\n'
   '            routing_plan,\n'
   '            durable_retry_claim,\n'
   '            minimal_response,\n'
   '            format,\n'
   '        )',
   'if response.status() == StatusCode::ACCEPTED\n'
   '            && let Some(reservation) = reservation\n'
   '        {\n'
   '            reservation.commit();\n'
   '        }',
   'Ok(response)',
   'queue_plan_synced_transport_unavailable')),
 ('crates/iroha_torii/src/lib.rs',
  'fn',
  'execute_torii_transaction_via_proxy',
  ("accepted_transaction: iroha_core::tx::AcceptedTransaction<'static>",
   'if transaction.admission_intent() != TransactionAdmissionIntent::QueuePlanSynced {\n'
   '        return threshold_key_lifecycle_ingress::submit(\n'
   '            app.clone(),\n'
   '            accepted_transaction,\n'
   '            routing_plan,\n'
   '            minimal_response,\n'
   '            format,\n'
   '        )\n'
   '        .await;\n'
   '    }',
   'durable_retry_claim.filter(|claim| claim.global_admission_identity.is_some())',
   'let already_durably_admitted = durable_retry_claim.is_some();',
   'if durable_retry_claim.is_none() {',
   'AuthenticatedQueuePlanRetry::from_accepted(\n'
   '            app.state.network_id_ref(),\n'
   '            &accepted_transaction,\n'
   '        )',
   'canonical_queue_plan_submission_response(',
   'queue_plan_synced_proxy_request_id_for_entrypoint(app.as_ref(), entrypoint_hash.clone())',
   'let binding = if let Some(claim) = durable_retry_claim {',
   'iroha_core::torii_proxy::queue_plan_binding_from_durable_admission(&claim)',
   'if binding.request_id != request_id {',
   '.plan_admission_context_with_state(app.state.as_ref(), &routing_plan)',
   'iroha_core::torii_proxy::validate_queue_plan_binding_for_request(\n'
   '        &binding,\n'
   '        app.state.network_id_ref(),\n'
   '        &transaction,\n'
   '        &routing_plan,\n'
   '    )',
   '.queue_plan_admission_binding_registry_match(&binding)',
   'Ok(QueuePlanAdmissionRegistryMatch::Exact) => {\n'
   '            return transaction_submission_response(',
   'Ok(QueuePlanAdmissionRegistryMatch::Conflict) => {\n'
   '            return queue_plan_admission_registry_conflict_response(',
   'Ok(QueuePlanAdmissionRegistryMatch::Absent) => {}',
   'if !already_durably_admitted {\n'
   '        if let Err(error) = routing::reject_ingress_if_queue_capacity_saturated(\n'
   '            app.queue.as_ref(),\n'
   '            app.state.as_ref(),\n'
   '            1,\n'
   '        ) {\n'
   '            return error.into_response();\n'
   '        }\n'
   '    }',
   'let response = execute_torii_proxy_request_with_fallback(\n'
   '        app,\n'
   '        routing_decision,\n'
   '        ToriiProxyRequestKindV1::SubmitTransaction {\n'
   '            transaction,\n'
   '            expected_plan: ToriiRoutingPlanHintV1::from(routing_plan),\n'
   '            admission: ToriiProxyTransactionAdmissionV1::QueuePlanSynced,\n'
   '            admission_binding: Some(binding),\n'
   '        },\n'
   '    )')),
 ('crates/iroha_torii/src/lib_pipeline_handlers.rs',
 'enum',
 'PreparedBatchEntry',
 ('Canonical(Response)', 'Fresh(PreparedFreshTransactionIngress)')),
 ('crates/iroha_torii/src/lib_pipeline_handlers.rs',
 'fn',
 'transaction_batch_submission_response',
 ('*response.status_mut() = StatusCode::ACCEPTED;',
  'HeaderValue::from_str(&accepted_count.to_string())')),
 ('crates/iroha_torii/src/lib_pipeline_handlers.rs',
  'fn',
  'handler_post_transactions_batch',
  ('validate_transaction_batch_body_size(&body, app.transaction_batch_max_bytes)?;',
   'move || decode_transaction_batch_request(body, max_transactions)',
   'admit_transaction_api_token_preauth(&app.tx_preauth_rate_limiter, token, '
   'transactions.len())',
   'precheck_transaction_batch_ed25519(\n'
   '                &transactions,\n'
   '                worker_app.state.pipeline.signature_batch_max_ed25519,\n'
   '            )',
   'for (transaction, precheck) in transactions.into_iter().zip(prechecks) {',
   'let hash = transaction.hash();',
   'AuthenticatedQueuePlanRetry::from_signed(\n'
   '                    worker_app.state.network_id_ref(),\n'
   '                    transaction.signed(),\n'
   '                )?',
   'canonical_queue_plan_submission_response(\n'
   '                    &worker_app,\n'
   '                    &authenticated,\n'
   '                    true,\n'
   '                    ResponseFormat::Json,\n'
   '                )',
   'prepared.push((hash, PreparedTransactionIngress::Canonical(response)));\n'
   '                    continue;',
   'routing::accept_decoded_signed_transaction_for_ingress_with_precheck(\n'
   '                        worker_app.state.clone(),\n'
   '                        transaction,\n'
   '                        &worker_app.telemetry,\n'
   '                        precheck.single_ed25519_prechecked,\n'
   '                        precheck.precheck_rejection,\n'
   '                    )?',
   'prepared.push((hash, PreparedTransactionIngress::Fresh(accepted)));\n'
   '            }\n'
   '            // Keep route/policy preflight before the first durable write. Ordinary\n'
   '            // inputs have exactly the same authenticated lifecycle exception.\n'
   '            prepared\n'
   '                .into_iter()\n'
   '                .map(|(hash, prepared)| {',
   'PreparedTransactionIngress::Canonical(response) => {\n'
   '                            PreparedBatchEntry::Canonical(response)\n'
   '                        }',
   'prepare_fresh_transaction_ingress(&worker_app, transaction)?;',
   'if prepared.transaction.entrypoint().admission_intent()\n'
   '                                != TransactionAdmissionIntent::QueuePlanSynced\n'
   '                            {\n'
   '                                threshold_key_lifecycle_ingress::authenticate(\n'
   '                                    &worker_app,\n'
   '                                    prepared.transaction.entrypoint(),\n'
   '                                    &prepared.routing_plan,\n'
   '                                )',
   'PreparedBatchEntry::Fresh(prepared)',
   'Ok((hash, prepared))\n'
   '                })\n'
   '                .collect::<Result<Vec<_>, Error>>()\n'
   '        },\n'
   '    )\n'
   '    .await?;\n'
   '    drop(permit);',
   'let mut outcomes = Vec::with_capacity(prepared.len());\n'
   '    for (hash, entry) in prepared {',
   'PreparedBatchEntry::Canonical(response) => response,',
   'if tokio::time::Instant::now() >= deadline {',
   'tokio::time::timeout_at(\n'
   '                            deadline,\n'
   '                            submit_prepared_transaction_ingress(\n'
   '                                &app,\n'
   '                                prepared,\n'
   '                                true,\n'
   '                                ResponseFormat::Json,\n'
   '                            ),\n'
   '                        )',
   'Ok(result) => result.unwrap_or_else(IntoResponse::into_response)',
   'Err(_) => queue_plan_outcome_unknown_response(\n'
   '                                entrypoint_hash,\n'
   '                                Some(hash),\n'
   '                                "batch deadline elapsed after this entry was '
   'dispatched",\n'
   '                            )',
   'outcomes.push(TransactionBatchEntryOutcome {\n'
   '            signed_transaction_hash: hash,\n'
   '            status: response.status().as_u16(),\n'
   '            reject_code: response\n'
   '                .headers()',
   'let accepted = outcomes.iter().filter(|entry| entry.status == 202).count();',
   'if accepted == outcomes.len() {\n'
   '        return Ok(transaction_batch_submission_response(accepted));\n'
   '    }',
   'let mut response = crate::utils::JsonBody(outcomes).into_response();\n'
   '    *response.status_mut() = StatusCode::MULTI_STATUS;',
   'HeaderValue::from_str(&accepted.to_string()).expect(')),
 ('crates/iroha_torii/src/lib.rs',
  'fn',
  'execute_incoming_torii_proxy_request_with_admission_inner',
  ('queue_plan_service_input_capacity_error(\n'
 '                app,\n'
 '                &transaction,\n'
 '                &admission_binding,\n'
 '                execution_deadline,\n'
 '                request_head.deadline_unix_ms,\n'
 '            )\n'
 '            .await',
 'push_accepted_transaction_for_ingress_with_routing_plan_strict_durable_claim(',
 'queue_plan_synced_admission_response(',
 'if transaction.admission_intent() != TransactionAdmissionIntent::QueuePlanSynced {',
 'let accepted_tx = match routing::accept_transaction_for_ingress(',
 'if admission_binding.request_id != request_head.request_id {',
 'if admission_binding.request_id != canonical_request_id {',
 'iroha_core::torii_proxy::validate_queue_plan_binding_for_request(\n'
 '                &admission_binding,\n'
 '                app.state.network_id_ref(),\n'
 '                &transaction,\n'
 '                &ingress_plan,\n'
 '            )',
 '.route_plan_with_state(&accepted_tx, app.state.as_ref())',
 QUEUE_PLAN_PROXY_CANONICAL_RESPONSE,
 'execution_deadline: tokio::time::Instant',
 'let authenticated = match AuthenticatedQueuePlanRetry::from_entrypoint(',
 'authenticated.entrypoint_hash()')),
 ('crates/iroha_torii/src/lib.rs',
  'fn',
  'transaction_submission_receipt_response',
  ('let mut response = if minimal_response {',
   '*response.status_mut() = StatusCode::ACCEPTED;',
   'TransactionSubmissionReceipt::try_sign(payload, &app.da_receipt_signer)',
   'insert_transaction_submission_identity_headers(')),
 ('crates/iroha_core/src/state.rs',
  'method',
  'State::canonical_queue_plan_input_read_working_set_bytes',
  ('let kura = crate::kura::canonical_admission_read_decode_limits()?;',
   'let complete = Self::canonical_queue_plan_input_decode_limits()?;',
   'let state_graph = complete\n'
   '            .max_total_allocated_bytes()\n'
   '            .checked_sub(kura.max_total_allocated_bytes())?;',
   'crate::kura::canonical_admission_read_working_set_bytes()?.checked_add(state_graph)')),
 ('crates/iroha_core/src/kura/lane_admission_source.rs',
  'fn',
  'canonical_admission_read_decode_limits',
  ('norito::canonical_decode_limits(',
   'usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES).ok()?',
   'norito::canonical_decode_limits(MAX_KURA_V2_FINALITY_RECORD_BYTES)',
   'norito::canonical_decode_limits(MAX_RETAINED_BLOCK_RECORD_BYTES)',
   'iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1',
   'iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES',
   'usize::try_from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1)\n'
   '            .ok()?\n'
   '            .checked_mul(2)?',
   'let operations = [\n'
   '        (body, 1usize),\n'
   '        (finality, 2),\n'
   '        (retained, 2),\n'
   '        (sccp, sccp_reads),\n'
   '        (input, 1),\n'
   '    ];',
   'elements.checked_add(limits.max_total_elements().checked_mul(count)?)?',
   'allocated.checked_add(limits.max_total_allocated_bytes().checked_mul(count)?)?',
   'Some(norito::DecodeLimits::new(\n'
   '        body.max_sequence_elements(),\n'
   '        body.max_field_bytes(),\n'
   '        elements,\n'
   '        allocated,\n'
   '        body.max_nesting_depth(),\n'
   '    ))')),
 ('crates/iroha_core/src/kura/lane_admission_source.rs',
  'fn',
  'canonical_admission_read_working_set_bytes',
  ('usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES).ok()?',
   'canonical_admission_read_decode_limits()?.max_total_allocated_bytes()',
   'MAX_KURA_V2_FINALITY_RECORD_BYTES.checked_add(MAX_RETAINED_BLOCK_RECORD_BYTES)?',
   'MAX_KURA_V2_FINALITY_RECORD_BYTES\n        .checked_next_power_of_two()?',
   '.checked_add(MAX_RETAINED_BLOCK_RECORD_BYTES.checked_next_power_of_two()?)?',
   'norito::canonical_decode_limits(input).max_total_allocated_bytes()',
   '.try_fold(0usize, usize::checked_add)',
   'let proposal_clone = norito::canonical_decode_limits(wire).max_total_allocated_bytes();',
   'proposal_clone,',
   'decoded,')),
 ('crates/iroha_data_model/src/block/mod.rs',
  'fn',
  'decode_framed_versioned_signed_block_inner',
  ('let block = view.decode::<SignedBlock>().map_err(VersionError::from)?;',
   'norito::core::DecodeFlagsGuard::enter(default_encode_flags())',
   'norito::core::encoded_payload_len(&block)',
   '.checked_add(1 + norito::core::Header::SIZE)',
   'if canonical_len != raw_for_error.len() {',
   'let canonical = block\n        .canonical_wire()',
   'if canonical.as_framed() != raw_for_error {')),
 ('crates/iroha_core/src/kura.rs',
  'method',
  'Kura::decode_v2_finality_record_at',
  ('self.read_regular_sidecar_snapshot(path, directory, MAX_KURA_V2_FINALITY_RECORD_BYTES)?',
   'norito::core::encoded_payload_len(&record)?',
   'if canonical_len != snapshot.bytes.len() || record.encode() != snapshot.bytes {')),
 ('crates/iroha_core/src/kura/retained_finality_replica_authority.rs',
  'method',
  'Kura::decode_canonical_retained_block_record',
  ('norito::core::encoded_payload_len(record).ok()',
   'record.format_version == RETAINED_BLOCK_RECORD_VERSION',
   '&& canonical_len == Some(bytes.len())',
   '&& record.encode() == bytes')),
 ('crates/iroha_torii/src/torii_fanout_decode_helpers.rs',
  'fn',
  'torii_proxy_strict_response_working_set_bytes',
  ('.checked_add(\n'
   '                '
   'iroha_core::state::State::canonical_queue_plan_input_read_working_set_bytes()?,\n'
   '            )',)),
 ('crates/iroha_core/src/state.rs',
  'method',
  'State::canonical_queue_plan_input_decode_limits',
  ('let kura = crate::kura::canonical_admission_read_decode_limits()?;',
   'let compact = norito::canonical_decode_limits(MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES);',
   'let compact_reads = crate::native_amx::MAX_NATIVE_AMX_PLAN_LEGS.checked_add(2)?;',
   'let one_elements = compact\n'
   '            .max_total_elements()\n'
   '            .checked_mul(compact_reads)?\n'
   '            .checked_add(MAX_QUEUE_PLAN_PENDING_OBLIGATION_BYTES)?;',
   'let one_allocated = compact\n'
   '            .max_total_allocated_bytes()\n'
   '            .checked_mul(compact_reads)?\n'
   '            .checked_add(MAX_QUEUE_PLAN_PENDING_OBLIGATION_BYTES.checked_mul(4)?)?;',
   'kura.max_total_elements()\n                .checked_add(one_elements.checked_mul(2)?)?',
   'kura.max_total_allocated_bytes()\n'
   '                .checked_add(one_allocated.checked_mul(2)?)?')),
 QUEUE_PLAN_PROXY_DEADLINE_BINDING,
 ('crates/iroha_torii/src/queue_plan_retry_authentication.rs',
  'struct',
  'AuthenticatedQueuePlanRetry',
  ('entrypoint_hash: HashOf<TransactionEntrypoint>',
   'signed_transaction_hash: HashOf<SignedTransaction>')),
 ('crates/iroha_torii/src/queue_plan_retry_authentication.rs',
  'method',
  'AuthenticatedQueuePlanRetry::from_signed',
  ('if signed.admission_intent() != TransactionAdmissionIntent::QueuePlanSynced {\n'
   '            return Ok(None);\n'
   '        }',
   'Self::check_network(network_id, signed)?;',
   'signed.verify_signature().map_err(|error| {',
   'entrypoint_hash: signed.hash_as_entrypoint(),',
   'signed_transaction_hash: signed.hash()')),
 ('crates/iroha_torii/src/queue_plan_retry_authentication.rs',
  'method',
  'AuthenticatedQueuePlanRetry::from_entrypoint',
  ('TransactionEntrypoint::External(signed) => signed,',
   'TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction(),',
   'TransactionEntrypoint::SealedCommitment(_) => return Ok(None)',
   'Self::from_signed(network_id, signed)?',
   'authenticated.entrypoint_hash = entrypoint.hash();')),
 ('crates/iroha_torii/src/queue_plan_retry_authentication.rs',
  'method',
  'AuthenticatedQueuePlanRetry::from_accepted',
  ("accepted: &AcceptedTransaction<'_>",
   'let entrypoint = accepted.entrypoint();',
   'if entrypoint.admission_intent() != TransactionAdmissionIntent::QueuePlanSynced {\n'
   '            return Ok(None);\n'
   '        }',
   'Self::check_network(network_id, signed)?;',
   'entrypoint_hash: entrypoint.hash(),',
   'signed_transaction_hash: signed.hash()')),
 ('crates/iroha_torii/src/queue_plan_retry_authentication.rs',
  'method',
  'AuthenticatedQueuePlanRetry::check_network',
  ('let expected = TransactionDomain::Network(*network_id);',
   'if signed.domain() != &expected {',
   'AcceptTransactionFail::TransactionDomainMismatch(Mismatch {',
   'actual: *signed.domain()')))

QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS = _merge_retained_queue_plan_bindings(
    QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS, QUEUE_PLAN_CANONICAL_RETRY_BINDINGS
)


def validate_canonical_queue_plan_retry(items: dict, errors: list[str]) -> None:
    """Bind exact source authentication and validation before the no-new-promise retry."""
    code_items = {}
    for path, kind, symbol, obligations in QUEUE_PLAN_CANONICAL_RETRY_BINDINGS:
        item = items.get((path, kind, symbol))
        if item is None:
            errors.append(f"{symbol}: missing canonical QueuePlan retry owner")
            continue
        code_items[symbol] = _code(item)
        for obligation in obligations:
            # This pre-existing diagnostic spelling remains an exact raw ledger
            # obligation above; it is not an executable authority predicate.
            if obligation == "queue_plan_synced_transport_unavailable":
                if obligation not in item:
                    errors.append(f"{symbol}: canonical QueuePlan diagnostic spelling changed")
                continue
            if _code(obligation) not in code_items[symbol]:
                errors.append(f"{symbol}: canonical QueuePlan retry relation changed: {obligation!r}")

    def ordered(symbol, *relations):
        code = code_items.get(symbol, "")
        cursor = 0
        for relation in relations:
            normalized = _code(relation)
            found = code.find(normalized, cursor)
            if found < 0:
                errors.append(f"{symbol}: canonical QueuePlan retry order changed: {relation!r}")
                break
            cursor = found + len(normalized)

    ordered("State::canonical_queue_plan_admitted_input",
            "let observe =", "let view = self.view();", "Ok(Some((record, carrier_hash)))", "};",
            "let Some((record, carrier_hash)) = observe()?", ".read_first_admission_carrier(",
            "decode_and_validate_lane_admitted_input_v1(",
            "if observe()?.as_ref() != Some(&(record, carrier_hash))", "result.map(Some)")
    for symbol, accept in (
        ("submit_signed_transaction_for_ingress_queue_plan_certified", "accept_decoded_signed_transaction_for_ingress("),
        ("handler_post_transaction_entrypoint", "accept_transaction_for_ingress("),
    ):
        authenticate = "AuthenticatedQueuePlanRetry::from_signed(" if symbol.startswith("submit_") else "AuthenticatedQueuePlanRetry::from_entrypoint("
        ordered(symbol, "run_transaction_ingress_compute_job(", authenticate,
                "canonical_queue_plan_submission_response(",
                "return Ok(PreparedTransactionIngress::Canonical(response));", accept,
                "drop(compute_permit);", "PreparedTransactionIngress::Canonical(response) => return Ok(response)",
                "let prepared = prepare_fresh_transaction_ingress(&app, accepted_tx)?;",
                "submit_prepared_transaction_ingress(")
    ordered("prepare_fresh_transaction_ingress",
            ".durable_plan_admission_claim_with_state(&transaction, app.state.as_ref())",
            "if !durable_retry_claim", "claim.global_admission_identity.is_some()",
            "reject_ingress_if_queue_capacity_saturated(",
            "if let Some(claim) = &durable_retry_claim", "claim.routing_plan.clone()",
            ".route_plan_with_state(&transaction, app.state.as_ref())",
            "Ok(PreparedFreshTransactionIngress {")
    ordered("submit_prepared_transaction_ingress",
            "let PreparedFreshTransactionIngress {", "} = prepared;",
            "AuthenticatedQueuePlanRetry::from_accepted(",
            "canonical_queue_plan_submission_response(", "return Ok(response);",
            ".durable_plan_admission_claim_with_state(&transaction, app.state.as_ref())",
            ".or(durable_retry_claim);",
            ".map_or(routing_plan, |claim| claim.routing_plan.clone());",
            "claim.global_admission_identity.is_some()",
            "reserve_verified_transaction_authority(",
            "execute_torii_transaction_via_proxy(",
            "if response.status() == StatusCode::ACCEPTED",
            "reservation.commit();", "Ok(response)")
    ordered("execute_torii_transaction_via_proxy",
            "threshold_key_lifecycle_ingress::submit(",
            "durable_retry_claim.filter(|claim| claim.global_admission_identity.is_some())",
            "let already_durably_admitted = durable_retry_claim.is_some();",
            "AuthenticatedQueuePlanRetry::from_accepted(",
            "canonical_queue_plan_submission_response(",
            "let binding = if let Some(claim) = durable_retry_claim",
            "queue_plan_binding_from_durable_admission(&claim)",
            "if binding.request_id != request_id",
            "validate_queue_plan_binding_for_request(",
            "queue_plan_admission_binding_registry_match(&binding)",
            "if !already_durably_admitted",
            "reject_ingress_if_queue_capacity_saturated(",
            "execute_torii_proxy_request_with_fallback(")
    ordered("handler_post_transactions_batch",
            "validate_transaction_batch_body_size(", "decode_transaction_batch_request(",
            "admit_transaction_api_token_preauth(", "precheck_transaction_batch_ed25519(",
            "for (transaction, precheck) in transactions.into_iter().zip(prechecks)",
            "AuthenticatedQueuePlanRetry::from_signed(", "canonical_queue_plan_submission_response(",
            "accept_decoded_signed_transaction_for_ingress_with_precheck(",
            "prepared.push((hash, PreparedTransactionIngress::Fresh(accepted)));",
            "prepare_fresh_transaction_ingress(", "threshold_key_lifecycle_ingress::authenticate(",
            ".collect::<Result<Vec<_>, Error>>()", "drop(permit);",
            "for (hash, entry) in prepared", "submit_prepared_transaction_ingress(",
            "outcomes.push(TransactionBatchEntryOutcome", "let accepted = outcomes.iter()",
            "if accepted == outcomes.len()", "transaction_batch_submission_response(accepted)",
            "JsonBody(outcomes)", "StatusCode::MULTI_STATUS")
    batch = code_items.get("handler_post_transactions_batch", "")
    preflight, separator, dispatch = batch.partition(_code("drop(permit);"))
    if not separator or "submit_prepared_transaction_ingress(" in preflight:
        errors.append("handler_post_transactions_batch: dispatch escapes complete preflight")
    if "?" in dispatch or "returnErr(" in dispatch:
        errors.append("handler_post_transactions_batch: aggregate rejection hides per-entry dispatch outcomes")
    for forbidden in ("push_accepted_transaction", ".queue.push(", "routing::push_"):
        if forbidden in batch:
            errors.append("handler_post_transactions_batch: direct queue insertion bypasses the shared durable owner")
    ordered("execute_incoming_torii_proxy_request_with_admission_inner",
            "AuthenticatedQueuePlanRetry::from_entrypoint(",
            "if admission_binding.request_id != request_head.request_id", "if admission_binding.request_id != canonical_request_id",
            "validate_queue_plan_binding_for_request(", QUEUE_PLAN_PROXY_CANONICAL_RESPONSE,
            "queue_plan_service_input_capacity_error(", QUEUE_PLAN_PROXY_CANONICAL_RESPONSE,
            "let accepted_tx = match routing::accept_transaction_for_ingress(",
            ".route_plan_with_state(",
            "push_accepted_transaction_for_ingress_with_routing_plan_strict_durable_claim(")
    inner = code_items.get("execute_incoming_torii_proxy_request_with_admission_inner", "")
    if (inner.count("canonical_queue_plan_synced_response(") != 2
            or inner.count(_code(QUEUE_PLAN_PROXY_CANONICAL_RESPONSE)) != 2):
        errors.append("execute_incoming_torii_proxy_request_with_admission_inner: "
                      "canonical QueuePlan retry must retain both original deadline handoffs")
    ordered("AuthenticatedQueuePlanRetry::from_signed", "Self::check_network(network_id, signed)?;",
            "signed.verify_signature()", "Ok(Some(Self {")
    ordered("AuthenticatedQueuePlanRetry::from_entrypoint", "Self::from_signed(network_id, signed)?",
            "authenticated.entrypoint_hash = entrypoint.hash();", "Ok(Some(authenticated))")
    for symbol in ("AuthenticatedQueuePlanRetry::from_signed", "AuthenticatedQueuePlanRetry::from_entrypoint"):
        for forbidden in ("AcceptedTransaction::", "accept_transaction_for_ingress(", "into_accepted(",
                          "transaction_admission_limits(", "admission_snapshot(", "allowed_signing", "PrecheckedSingleEd25519"):
            if forbidden in code_items.get(symbol, ""):
                errors.append(f"{symbol}: retry authentication acquired fresh admission policy or authority")
    ordered("canonical_queue_plan_synced_response", "authenticated.entrypoint_hash() != binding.entrypoint_hash",
            "queue_plan_admission_binding_registry_match(binding)",
            "canonical_queue_plan_admitted_input(binding.entrypoint_hash)",
            "if &input.input().certificate.binding == binding", "if tokio::time::Instant::now() >= read_deadline", "NoritoBody(input.into_input().certificate)")
    for symbol in ("canonical_queue_plan_submission_response", "canonical_queue_plan_synced_response"):
        for forbidden in ("route_plan_with_state(", "push_accepted_transaction", "queue_plan_synced_admission_response("):
            if forbidden in code_items.get(symbol, ""):
                errors.append(f"{symbol}: canonical retry creates fresh route/admission authority")
    if "insert_routing_headers(" in code_items.get("transaction_submission_receipt_response", ""):
        errors.append("transaction_submission_receipt_response: canonical public receipt invents fresh routing")

    ordered("canonical_queue_plan_synced_response", "let reservation = match proxy_memory",
            "acquire_torii_proxy_memory(app)", "tokio::task::block_in_place(",
            "canonical_queue_plan_admitted_input(binding.entrypoint_hash)",
            "hold_torii_proxy_memory_in_response_body(")
    ordered("decode_framed_versioned_signed_block_inner", "view.decode::<SignedBlock>()",
            "encoded_payload_len(&block)", "if canonical_len != raw_for_error.len()",
            ".canonical_wire()", "if canonical.as_framed() != raw_for_error")
    for forbidden in ("spawn_blocking(", "tokio::spawn(", "std::thread::spawn("):
        if forbidden in code_items.get("canonical_queue_plan_synced_response", ""):
            errors.append("canonical_queue_plan_synced_response: detached canonical read loses physical W custody")
    if ".get_block(" in code_items.get("Kura::read_first_admission_carrier_under_prune_and_canonical_guards", ""):
        errors.append("Kura::read_first_admission_carrier: canonical read materializes an unowned cache body")

    ordered("Kura::read_first_admission_carrier", "let _prune = self.prune_lock.lock();",
            "ensure_prune_recovery_not_required()", "let _canonical = self.canonical_chain_lock.lock();",
            "read_first_admission_carrier_under_prune_and_canonical_guards(height, expected_hash)")
    guarded = code_items.get("Kura::read_first_admission_carrier_under_prune_and_canonical_guards", "")
    for forbidden in ("self.prune_lock.lock(", "self.canonical_chain_lock.lock("):
        if forbidden in guarded:
            errors.append("Kura::read_first_admission_carrier_under_prune_and_canonical_guards: guarded delegate reacquires an original fence")
    ordered("Kura::read_first_admission_carrier_under_prune_and_canonical_guards",
            "validate_v2_finality_record_at(", "retained_block_record_at_without_live_body(",
            "validate_v2_finality_wire_bindings(", "verify_v2_finality_artifact_at(",
            "drop(read_identity);", "read_block_body_under_prune_and_canonical_guards(height)")
    validate_queue_plan_proxy_deadline_owner(
        items.get(QUEUE_PLAN_PROXY_DEADLINE_BINDING[:3], ""), errors
    )


# Replay-terminal cleanup retains canonical evidence on the original claim.
# Queue release resumes that exact obligation; autonomous retirement still needs Kura Complete.
QUEUE_PLAN_REPLAY_TERMINAL_BINDINGS = (('crates/iroha_core/src/queue.rs',
  'enum',
  'QueuePlanLocalCustody',
  ('enum QueuePlanLocalCustody {\n'
   '    /// No autonomous reservation has taken this admission in the current process.\n'
   '    Available,\n'
   '    /// Autonomous ownership requires its checked direct release or Kura terminal proof.\n'
   '    Autonomous,\n'
   '    /// Canonical replay or closed-route evidence authorized cleanup, but a\n'
   '    /// selection or popped guard still owns the exact claim.\n'
   '    ReplayTerminalPending,\n'
   '}',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::replay_terminal_cleanup_pending',
  ('    fn replay_terminal_cleanup_pending(&self, hash: EntrypointHash) -> bool {\n'
   '        self.durable_plan_claims.get(&hash).is_some_and(|claim| {\n'
   '            claim.local_custody == QueuePlanLocalCustody::ReplayTerminalPending\n'
   '        })\n'
   '    }',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::resume_replay_terminal_cleanup',
  ('    fn resume_replay_terminal_cleanup(&self, hash: EntrypointHash) {\n'
   '        if self.transaction_selection_durability_faulted() {\n'
   '            return;\n'
   '        }\n'
   '        let claim = self.durable_plan_claims.get(&hash).and_then(|claim| {\n'
   '            (claim.local_custody == QueuePlanLocalCustody::ReplayTerminalPending)\n'
   '                .then(|| claim.value().clone())\n'
   '        });\n'
   '        let result = match claim {\n'
   '            Some(claim) => self.reject_unreserved_terminal_plan_claim(&claim),\n'
   '            None => return,\n'
   '        };\n'
   '        match result {\n'
   '            Ok(true) => self.publish_backpressure_state(self.active_len(), None),\n'
   '            Ok(false) => {}\n'
   '            Err(error) => {\n'
   '                self.mark_accepted_work_validation_fault(\n'
   '                    hash,\n'
   '                    "replay_terminal_owner_release",\n'
   '                    &error,\n'
   '                    None,\n'
   '                );\n'
   '            }\n'
   '        }\n'
   '    }',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::resume_unowned_replay_terminal_cleanup',
  ('    fn resume_unowned_replay_terminal_cleanup(&self) {\n'
   '        if self.inflight_guards.load(Ordering::Acquire) != 0\n'
   '            || self.selection_attempts.load(Ordering::Acquire) != 0\n'
   '            || self.transaction_selection_durability_faulted()\n'
   '        {\n'
   '            return;\n'
   '        }\n'
   '        // Clear before scanning: a concurrent new obligation sets the hint\n'
   '        // again, and a still-owned obligation does so when its retry defers.\n'
   '        // No normal guard release scans unrelated claims without such work.\n'
   '        if !self\n'
   '            .replay_terminal_cleanup_dirty\n'
   '            .swap(false, Ordering::AcqRel)\n'
   '        {\n'
   '            return;\n'
   '        }\n'
   '        let pending = self\n'
   '            .durable_plan_claims\n'
   '            .iter()\n'
   '            .filter_map(|claim| {\n'
   '                (claim.local_custody == QueuePlanLocalCustody::ReplayTerminalPending)\n'
   '                    .then_some(*claim.key())\n'
   '            })\n'
   '            .collect::<Vec<_>>();\n'
   '        for hash in pending {\n'
   '            self.resume_replay_terminal_cleanup(hash);\n'
   '        }\n'
   '    }',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'GlobalQueueSelectionLease::retain_only',
  ('    pub(crate) fn retain_only(&mut self, retained: &[EntrypointHash]) -> bool {\n'
   '        if self.owner == 0 {\n'
   '            return retained.is_empty();\n'
   '        }\n'
   '        let Some(queue) = self.queue.upgrade() else {\n'
   '            return false;\n'
   '        };\n'
   '        let retained_set = retained.iter().copied().collect::<HashSet<_>>();\n'
   '        let leased_set = self.hashes.iter().copied().collect::<HashSet<_>>();\n'
   '        let exact_subset = retained_set.len() == retained.len()\n'
   '            && leased_set.len() == self.hashes.len()\n'
   '            && retained_set.iter().all(|hash| leased_set.contains(hash));\n'
   '        let first_hash = self\n'
   '            .hashes\n'
   '            .first()\n'
   '            .copied()\n'
   '            .or_else(|| retained.first().copied());\n'
   '        let queue_guard = queue.push_remove_lock.lock();\n'
   '        let mut owners = queue.global_selection_owners.lock();\n'
   '        let ownership_intact = self\n'
   '            .hashes\n'
   '            .iter()\n'
   '            .all(|hash| owners.get(hash) == Some(&self.owner));\n'
   '        if !exact_subset || !ownership_intact {\n'
   '            drop(owners);\n'
   '            drop(queue_guard);\n'
   '            if let Some(hash) = first_hash {\n'
   '                queue.mark_accepted_work_validation_fault(\n'
   '                    hash,\n'
   '                    "global_candidate_selection",\n'
   '                    "global candidate selection lease changed before exact narrowing",\n'
   '                    None,\n'
   '                );\n'
   '            }\n'
   '            return false;\n'
   '        }\n'
   '        let mut released = Vec::new();\n'
   '        for hash in &self.hashes {\n'
   '            if !retained_set.contains(hash) {\n'
   '                owners.remove(hash);\n'
   '                released.push(*hash);\n'
   '            }\n'
   '        }\n'
   '        self.hashes.retain(|hash| retained_set.contains(hash));\n'
   '        drop(owners);\n'
   '        drop(queue_guard);\n'
   '        for hash in released {\n'
   '            queue.resume_replay_terminal_cleanup(hash);\n'
   '        }\n'
   '        !queue.transaction_selection_durability_faulted()\n'
   '    }',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'GlobalQueueSelectionLease::drop',
  ('    fn drop(&mut self) {\n'
   '        if self.owner == 0 {\n'
   '            return;\n'
   '        }\n'
   '        let Some(queue) = self.queue.upgrade() else {\n'
   '            return;\n'
   '        };\n'
   '        let queue_guard = queue.push_remove_lock.lock();\n'
   '        let mut owners = queue.global_selection_owners.lock();\n'
   '        for hash in &self.hashes {\n'
   '            if owners.get(hash) == Some(&self.owner) {\n'
   '                owners.remove(hash);\n'
   '            }\n'
   '        }\n'
   '        drop(owners);\n'
   '        drop(queue_guard);\n'
   '        for hash in &self.hashes {\n'
   '            queue.resume_replay_terminal_cleanup(*hash);\n'
   '        }\n'
   '    }',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  "QueueSelectionAttempt<'_>::drop",
  ('    fn drop(&mut self) {\n'
   '        let previous = self.queue.selection_attempts.fetch_sub(1, Ordering::AcqRel);\n'
   '        debug_assert!(previous > 0, "queue selection-attempt counter underflow");\n'
   '        if previous == 1 {\n'
   '            self.queue.resume_unowned_replay_terminal_cleanup();\n'
   '        }\n'
   '    }',)),
 ('crates/iroha_core/src/queue.rs',
  'struct',
  'QueuePlanDurableClaimIndexEntry',
  ('local_custody: QueuePlanLocalCustody,',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::reject_exact_queue_plan_admission_claim_inner',
  ('if require_unreserved_replay_terminal_owner\n'
   '                && self.transaction_selection_durability_faulted()\n'
   '            {\n'
   '                return Err(LaneQueueReservationError::DurabilityFault);\n'
   '            }',
   'if &indexed_binding != binding {\n'
   '                // A delayed losing certificate must not delete a later admission for the same\n'
   '                // entrypoint, including an ABA replacement with the same routing-plan digest.\n'
   '                return Ok(false);\n'
   '            }',
   'if reservation_owned\n'
   '                    || indexed_claim.local_custody == QueuePlanLocalCustody::Autonomous\n'
   '                {\n'
   '                    return Ok(false);\n'
   '                }',
   'self.durable_plan_claims\n'
   '                    .get_mut(&hash)\n'
   '                    .expect("the Queue lock retains the exact admission claim")\n'
   '                    .local_custody = QueuePlanLocalCustody::ReplayTerminalPending;',
   'self.replay_terminal_cleanup_dirty\n                    .store(true, Ordering::Release);',
   'if self.global_selection_owners.lock().contains_key(&hash)\n'
   '                    || self.inflight_guards.load(Ordering::Acquire) != 0\n'
   '                    || self.selection_attempts.load(Ordering::Acquire) != 0\n'
   '                {\n'
   '                    return Ok(false);\n'
   '                }')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'TransactionGuard::drop',
  ('self.queue.release_inflight_guard();\n'
   '        self.released = true;\n'
   '        self.queue.resume_unowned_replay_terminal_cleanup();',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::reserve_transactions_for_lane_bounded',
  ('if self.replay_terminal_cleanup_pending(hash) {\n                continue;\n            }',
   'self.durable_plan_claims\n'
   '                .get_mut(&record.key.entrypoint_hash)\n'
   '                .expect("the reservation transition retains its validated admission claim")\n'
   '                .local_custody = QueuePlanLocalCustody::Autonomous;')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::release_pre_kura_autonomous_reservation_batch',
  ('let records = self.revalidate_complete_live_pre_kura_group_locked(expected_group, keys)?;',
   'let authorized_projection = checked.into_projection();',
   'self.durable_plan_claims\n'
   '                .get_mut(&record.key.entrypoint_hash)\n'
   '                .expect("the pre-Kura release retains its validated admission claim")\n'
   '                .local_custody = QueuePlanLocalCustody::Available;')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::prepare_plan_journal_replay_locked',
  ('if has_durable_reservation_owner {\n'
   '                claim.local_custody = QueuePlanLocalCustody::Autonomous;\n'
   '            }',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::push_with_lane_internal_with_state_and_routing',
  ('if existing.local_custody == QueuePlanLocalCustody::ReplayTerminalPending {\n'
   '                    return Err(Failure {\n'
   '                        tx: tx.into(),\n'
   '                        err: Error::InBlockchain,\n'
   '                    });\n'
   '                }',
   'local_custody: existing.local_custody,')),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::enqueue_prepared_admissions',
  ('local_custody: if restored_reservation {\n'
   '                            QueuePlanLocalCustody::Autonomous\n'
   '                        } else {\n'
   '                            QueuePlanLocalCustody::Available\n'
   '                        },',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::bounded_pending_snapshot',
  ('if self.replay_terminal_cleanup_pending(*hash) {\n                    return None;\n                }',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::pop_queued_hash',
  ('if self.durability_transition_active(&hash)\n'
   '                    || self.replay_terminal_cleanup_pending(hash)\n'
   '                {',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::begin_selection_attempt',
  ('self.selection_attempts.fetch_add(1, Ordering::AcqRel);\n'
   '        QueueSelectionAttempt { queue: self }',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::release_inflight_guard',
  ('self.inflight_guards.fetch_sub(1, Ordering::Relaxed)',)),
 ('crates/iroha_core/src/queue.rs',
  'method',
  'Queue::remove_state_committed_replay_owners_preserving_globally_bound',
  ('if registry_match == QueuePlanAdmissionRegistryMatch::Exact {',
   'Ok(evidence) if evidence == expected_evidence',
   'self.reject_unreserved_replay_terminal_queue_plan_admission_claim(&binding)?')),
 ('crates/iroha_core/src/sumeragi/v2_lane_work.rs',
  'method',
  'V2LaneWorkAdapter::release_pending_autonomous_reservation_batches',
  ('    fn release_pending_autonomous_reservation_batches(&mut self) -> Result<usize, V2LaneWorkError> {\n'
   '        if self.pending_autonomous_reservation_batches.is_empty() {\n'
   '            return Ok(0);\n'
   '        }\n'
   '        let queue = self.lane_drain_queue.as_ref().ok_or_else(|| {\n'
   '            V2LaneWorkError::InvalidContext(\n'
   '                "autonomous reservation release requires the installed live queue".to_owned(),\n'
   '            )\n'
   '        })?;\n'
   '        let mut released = 0_usize;\n'
   '        while let Some((&route, batch)) = self\n'
   '            .pending_autonomous_reservation_batches\n'
   '            .first_key_value()\n'
   '        {\n'
   '            if !batch.reservations.is_empty() {\n'
   '                let context = batch.pre_kura_direct_release_context()?;\n'
   '                released = released.saturating_add(\n'
   '                    queue\n'
   '                        .release_pre_kura_autonomous_reservation_batch(context)\n'
   '                        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?,\n'
   '                );\n'
   '            }\n'
   '            // A refused or indeterminate release retains this original batch\n'
   "            // and every unvisited batch. Only Queue's completed transition\n"
   "            // discharges the adapter's custody; no reconstructed retry owner.\n"
   '            self.pending_autonomous_reservation_batches.remove(&route);\n'
   '        }\n'
   '        Ok(released)\n'
   '    }',)))

# Shared startup/live owners have one ledger row. Preserve every startup
# obligation and append the retained-custody relations to that same declaration.
_QUEUE_PLAN_REPLAY_STARTUP_KEYS = {row[:3] for row in QUEUE_PLAN_STARTUP_REPLAY_BINDINGS}
QUEUE_PLAN_STARTUP_REPLAY_BINDINGS = _merge_retained_queue_plan_bindings(
    QUEUE_PLAN_STARTUP_REPLAY_BINDINGS,
    tuple(row for row in QUEUE_PLAN_REPLAY_TERMINAL_BINDINGS
          if row[:3] in _QUEUE_PLAN_REPLAY_STARTUP_KEYS),
)
_QUEUE_PLAN_REPLAY_STARTUP_ROWS = {row[:3]: row for row in QUEUE_PLAN_STARTUP_REPLAY_BINDINGS}
QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS = _merge_retained_queue_plan_bindings(
    QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS,
    tuple(_QUEUE_PLAN_REPLAY_STARTUP_ROWS.get(row[:3], row)
          for row in QUEUE_PLAN_REPLAY_TERMINAL_BINDINGS),
)

def validate_queue_plan_replay_terminal_custody(items: dict, errors: list[str]) -> None:
    """Bind authenticated original-claim custody and release-driven progress."""
    code_items = {}
    for path, kind, symbol, obligations in QUEUE_PLAN_REPLAY_TERMINAL_BINDINGS:
        item = items.get((path, kind, symbol))
        if item is None:
            errors.append(f"{symbol}: missing replay-terminal custody owner")
            continue
        code_items[symbol] = _code(item)
        for obligation in obligations:
            if _code(obligation).rstrip(",") not in code_items[symbol]:
                errors.append(f"{symbol}: replay-terminal custody relation changed: {obligation!r}")

    def ordered(symbol: str, *relations: str) -> None:
        code = code_items.get(symbol, "")
        offset = 0
        for relation in relations:
            token = _code(relation)
            position = code.find(token, offset)
            if position < 0:
                errors.append(f"{symbol}: replay-terminal custody order changed: {relation!r}")
                return
            offset = position + len(token)

    ordered("Queue::reject_exact_queue_plan_admission_claim_inner",
            "let queue_guard = self.push_remove_lock.lock();",
            "if require_unreserved_replay_terminal_owner && self.transaction_selection_durability_faulted()",
            "self.wait_for_durability_transitions(&[hash]);",
            "if &indexed_binding != binding", "if require_unreserved_replay_terminal_owner {",
            "if reservation_owned || indexed_claim.local_custody == QueuePlanLocalCustody::Autonomous",
            ".local_custody = QueuePlanLocalCustody::ReplayTerminalPending;",
            "self.replay_terminal_cleanup_dirty.store(true, Ordering::Release);",
            "if self.global_selection_owners.lock().contains_key(&hash)",
            ".begin_durability_transition_locked([hash])",
            "self.tombstone_conflicting_global_admission(binding)?;",
            "self.finalize_conflicting_global_admission_locked(")
    ordered("Queue::push_with_lane_internal_with_state_and_routing",
            "if existing.local_custody == QueuePlanLocalCustody::ReplayTerminalPending",
            '.expect("active durable retry was checked under the queue lock")',
            "local_custody: existing.local_custody")
    ordered("Queue::reserve_transactions_for_lane_bounded",
            "if self.replay_terminal_cleanup_pending(hash)",
            "self.apply_lane_reservation_journal(",
            ".local_custody = QueuePlanLocalCustody::Autonomous;",
            "store.live_by_entrypoint.insert(record.key.entrypoint_hash, record.clone());")
    ordered("Queue::release_pre_kura_autonomous_reservation_batch",
            "self.revalidate_complete_live_pre_kura_group_locked(expected_group, keys)?;",
            "let authorized_projection = checked.into_projection();",
            "journal.release_batch(release_keys)",
            ".local_custody = QueuePlanLocalCustody::Available;")
    ordered("Queue::resume_replay_terminal_cleanup",
            "claim.local_custody == QueuePlanLocalCustody::ReplayTerminalPending",
            ".then(|| claim.value().clone())",
            "Some(claim) => self.reject_unreserved_terminal_plan_claim(&claim)",
            "Ok(true) => self.publish_backpressure_state(self.active_len(), None)")
    for symbol in ("Queue::resume_replay_terminal_cleanup", "Queue::resume_unowned_replay_terminal_cleanup",
                   "GlobalQueueSelectionLease::drop", "QueueSelectionAttempt<'_>::drop", "TransactionGuard::drop"):
        for forbidden in ("state.view(", "State::", "Kura::", "tokio::spawn(", "std::thread::spawn("):
            if _code(forbidden) in code_items.get(symbol, ""):
                errors.append(f"{symbol}: replay-terminal release acquired replacement authority or a scheduler")
    if "resume_unowned_replay_terminal_cleanup" in code_items.get("Queue::release_inflight_guard", ""):
        errors.append("Queue::release_inflight_guard: replay-terminal retry can reenter held Queue locks")
