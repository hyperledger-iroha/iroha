# Executed lexically before the successor recovery source contracts; do not import directly.


def _successor_recovery_lifecycle_source_fidelity_errors(
    adapter_path,
    adapter_source,
    adjacent_store_path,
    adjacent_store_source,
    apply_path,
    apply_source,
    body_pipeline_path,
    body_pipeline_source,
    body_store_path,
    body_store_source,
    concrete_admission_path,
    concrete_admission_source,
    effects_path,
    effects_source,
    errors,
    finalized_output_source,
    ingress_position_path,
    ingress_position_source,
    kura_path,
    kura_source,
    launch_path,
    launch_source,
    ledger_operations_path,
    ledger_operations_source,
    ledger_path,
    ledger_source,
    ledger_store_path,
    ledger_store_source,
    lifecycle_open_path,
    lifecycle_open_source,
    lifecycle_projection_path,
    lifecycle_projection_source,
    lifecycle_startup_test_source,
    owner_path,
    owner_source,
    region,
    registry_path,
    registry_source,
    registry_validate_impl_path,
    registry_validate_impl_source,
    registry_validate_path,
    registry_validate_source,
    reject_tokens,
    replay_authority_path,
    replay_authority_source,
    require_literal_count,
    require_order,
    require_token_count,
    require_tokens,
    runner_dependency_path,
    runner_dependency_source,
    runtime_path,
    runtime_source,
    safety_wal_path,
    safety_wal_source,
    scheduler_path,
    scheduler_source,
    schema_path,
    schema_source,
    selector_path,
    selector_source,
    snapshot_path,
    snapshot_source,
    state_path,
    state_source,
    sumeragi_path,
    sumeragi_source,
    transport_path,
    transport_source,
    turn_driver_path,
    turn_driver_source,
    wal_recovery_path,
    wal_recovery_source,
    worker_path,
    worker_source,
) -> None:
        if (
            launch_source
            and turn_driver_source
            and kura_source
            and owner_source
            and worker_source
            and scheduler_source
            and registry_source
            and registry_validate_source
            and concrete_admission_source
            and lifecycle_projection_source
            and wal_recovery_source
            and selector_source
            and body_pipeline_source
            and replay_authority_source
            and runtime_source
            and effects_source
            and transport_source
            and lifecycle_open_source
            and runner_dependency_source
            and finalized_output_source
            and lifecycle_startup_test_source
            and state_source
            and snapshot_source
            and schema_source
            and apply_source
        ):
            cold_validate_owner = region(
                registry_path,
                registry_source,
                "move-only cold Ready Validate retry owner",
                "pub(in crate::sumeragi) struct RecoveredDurableValidateRetryOwnerV1 {",
                "/// Closed failure while joining cold Validate retry authority at launch.",
            )
            require_tokens(
                registry_path,
                "move-only cold Ready Validate retry owner",
                cold_validate_owner,
                (
                    "expected_decision: Option<(wire::ConsensusRound, wire::ConsensusRound, wire::BlockSubject, wire::ExecutionCommitment,)>",
                    "effect: AdapterEffect",
                    "durable_receipt: DurableBodyReceipt",
                    "binding: RecoveredDurableValidateRetryBindingV1",
                    "fn key(",
                    "fn bind_validated_marker(",
                    "fn exactly_matches_validated_marker(",
                    "self.key() == key",
                    "validated.durable() == &self.durable_receipt",
                    "bind_validated_marker_commitment(validated.execution_commitment())",
                    "exactly_matches_validated_marker_commitment(",
                    "fn initial_retry_frontier(",
                    "fn exactly_matches_retry(",
                    "frontier: &RecoveredDurableValidateRetryFrontierV1",
                    ".project_retry(&self.effect, frontier, effect, incoming)",
                ),
            )
            reject_tokens(
                registry_path,
                "move-only cold Ready Validate retry owner",
                cold_validate_owner,
                ("derive(Clone", "fn into_parts(", "fn into_ownership("),
            )
            cold_validate_census = region(
                registry_path,
                registry_source,
                "opaque complete cold Ready Validate retry census",
                "pub(in crate::sumeragi) struct RecoveredDurableValidateRetryCensusV1 {",
                "/// Closed failure while joining cold Validate retry authority at launch.",
            )
            require_tokens(
                registry_path,
                "opaque complete cold Ready Validate retry census",
                cold_validate_census,
                (
                    "owners: BTreeMap<",
                    "fn classify_and_bind_validated_marker(",
                    "self.owners.get_mut(&key)",
                    "owner.bind_validated_marker(key, validated)",
                    "fn install_into_executor",
                    "for owner in self.owners.into_values()",
                ),
            )
            reject_tokens(
                registry_path,
                "opaque complete cold Ready Validate retry census",
                cold_validate_census,
                ("derive(Clone", "fn iter(", "fn into_parts(", "Vec<"),
            )
            cold_validate_projection = _require_rust_item(
                registry_validate_impl_path,
                registry_validate_impl_source,
                "project_recovered_durable_validate_retry_census",
                errors,
            )
            if cold_validate_projection is not None:
                require_order(
                    registry_validate_impl_path,
                    "complete cold Ready Validate owner census",
                    cold_validate_projection.source,
                    (
                        "coordinator.fault.is_some() || coordinator.active_lease.is_some()",
                        "if let Some((decision_round, proposal_round, _, _)) = decision",
                        "decision_round != proposal_round",
                        "let mut logical_keys = std::collections::BTreeSet::new()",
                        "for work in self.entries.values()",
                        "ConcreteLifecycleWorkKind::DurableValidateBody(validate)",
                        "candidate_statement()",
                        "logical_keys.insert((statement.proposal_round(), subject))",
                        "RecoveredDurableValidateRetryOwnerErrorV1::MultipleCarriers",
                        "let mut owners = BTreeMap::new()",
                        "for (address, work) in &self.entries",
                        "let expected_key = LifecycleKey::new(",
                        "let matching_decision = decision.filter(",
                        "Some(wire::GlobalPhase::Commit), Some(commitment)",
                        "decision != Some(carrier_decision)",
                        "Some(wire::GlobalPhase::Prepare), Some(_)",
                        "AdapterEffect::ValidateBody",
                        "record.state != super::LifecycleState::Ready",
                        "record.work_class != LifecycleWorkClass::Validate",
                        "record.key != expected_key",
                        "record.physical_slots != BTreeMap::from([(address.slot, work.digest)])",
                        "coordinator.ready_index.contains(&record.ordinal)",
                        "validate.matches_recovered_record(",
                        "project_recovered_durable_validate_retry_binding(",
                        "matching_decision",
                        "owners.insert(key, owner).is_some()",
                        "RecoveredDurableValidateRetryCensusV1",
                    ),
                )
            cold_validate_binding_region = region(
                runtime_path,
                runtime_source,
                "closed cold Ready Validate retry binding and frontier",
                "pub(in crate::sumeragi) struct RecoveredDurableValidateRetryBindingV1 {",
                "/// Mint the unique pending owner of one exact payload-free live-WAL continuation.",
            )
            require_tokens(
                runtime_path,
                "closed cold Ready Validate retry binding and frontier",
                cold_validate_binding_region,
                (
                    "authority_ceiling_commitment: Option<wire::ExecutionCommitment>",
                    "pub(in crate::sumeragi) struct RecoveredDurableValidateRetryFrontierV1",
                    "effect: AdapterEffect",
                    "statement: RuntimeCandidateSemanticStatement",
                    "fn bind_validated_marker_commitment(",
                    "self.authority_ceiling_commitment = Some(commitment)",
                    "fn exactly_matches_validated_marker_commitment(",
                    "fn initial_frontier(",
                    "authority_ceiling_commitment: self.authority_ceiling_commitment",
                    "fn project_retry(",
                ),
            )
            cold_validate_commitment_projection = _require_qualified_rust_item(
                runtime_path,
                runtime_source,
                "RecoveredDurableValidateRetryFrontierV1",
                "project_commitment_ceiling",
                errors,
                "pure recovered Validate durable commitment projection",
            )
            if cold_validate_commitment_projection is not None:
                require_order(
                    runtime_path,
                    "pure recovered Validate durable commitment projection",
                    cold_validate_commitment_projection.source,
                    (
                        "self.authority_ceiling_commitment.is_some_and(|expected| expected != commitment)",
                        "let mut projected = self.clone()",
                        "projected.authority_ceiling_commitment = Some(commitment)",
                        "Ok(projected)",
                    ),
                )
            pending_retry_binding = region(
                runtime_path,
                runtime_source,
                "ordinal-free recovered Validate pending binding",
                "pub(crate) struct PendingRuntimeEffectBinding {",
                "/// Move-only restart successor derived from one exact recovered WAL vote.",
            )
            require_tokens(
                runtime_path,
                "ordinal-free recovered Validate pending binding",
                pending_retry_binding,
                (
                    "causal_lifecycle_key: iroha_crypto::Hash",
                    "effect_kind: u8",
                    "candidate_kind: u8",
                    "projection_hash: iroha_crypto::Hash",
                ),
            )
            reject_tokens(
                runtime_path,
                "ordinal-free recovered Validate pending binding",
                pending_retry_binding,
                ("ordinal:", "lifecycle_ordinal:"),
            )
            cold_validate_binding = _require_qualified_rust_item(
                runtime_path,
                runtime_source,
                "RecoveredDurableValidateRetryBindingV1",
                "project_retry",
                errors,
                "exact cold Ready Validate retry binding",
            )
            if cold_validate_binding is not None:
                require_order(
                    runtime_path,
                    "exact cold Ready Validate retry binding",
                    cold_validate_binding.source,
                    (
                        "frontier_tag != recovered_tag",
                        "incoming_tag != frontier_tag",
                        "frontier_round != recovered_round",
                        "frontier_subject != recovered_subject",
                        "incoming_round != recovered_round",
                        "incoming_subject != recovered_subject",
                        "self.pending.validate_exact(recovered_effect)",
                        "incoming.validate_exact()",
                        "recovered_statement.commit_refinement_to(self.expected_retry_statement)",
                        "recovered_statement.body_stage_authority_relation_to(frontier.statement)",
                        "RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Upgrade",
                        "frontier.statement.body_stage_authority_relation_to(incoming_statement)",
                        "frontier.authority_ceiling_commitment.zip(incoming_commitment)",
                        "RuntimeFetchAuthorityRelation::Upgrade => incoming_statement",
                        "RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Stale",
                        "incoming_binding.effect_identity != effect_identity",
                        "candidate.statement != Some(incoming_statement)",
                        "RuntimeEffectOwnership::new_bound(",
                        "Ok((",
                        "effect: effect.clone()",
                        "statement: retained_statement",
                        ".authority_ceiling_commitment.or(incoming_commitment)",
                    ),
                )
                reject_tokens(
                    runtime_path,
                    "origin-neutral cold Ready Validate retry binding",
                    cold_validate_binding.source,
                    (
                        "incoming.exact_pending_adapter_effect_binding(effect)",
                        "incoming_pending.causal_lifecycle_key != self.pending.causal_lifecycle_key",
                    ),
                )
            cold_validate_seal = region(
                effects_path,
                effects_source,
                "lineage-separated durable Validate retry seal",
                "enum DurableValidateRetrySealV1 {",
                "struct DurableValidateRetryProjectionV1 {",
            )
            require_tokens(
                effects_path,
                "lineage-separated durable Validate retry seal",
                cold_validate_seal,
                (
                    "Live { effect: AdapterEffect, ownership: RuntimeEffectOwnership, store_terminal: Option<DurableStoreTerminalRetrySealV1>, lifecycle_ordinal: Option<u128>, }",
                    "Recovered { owner: Arc<RecoveredDurableValidateRetryOwnerV1>, frontier: RecoveredDurableValidateRetryFrontierV1, lifecycle_ordinal: Option<u128>, }",
                ),
            )
            reject_tokens(
                effects_path,
                "lineage-separated durable Validate retry seal",
                cold_validate_seal,
                ("Recovered(Arc<RecoveredDurableValidateRetryOwnerV1>)",),
            )
            for ordinal_method, ordinal_fragments, description in (
                (
                    "lifecycle_ordinal",
                    (
                        "Self::Live { lifecycle_ordinal, .. }",
                        "Self::Recovered { lifecycle_ordinal, .. }",
                        "*lifecycle_ordinal",
                    ),
                    "exact durable Validate retry lifecycle row projection",
                ),
                (
                    "bind_lifecycle_ordinal",
                    (
                        "if ordinal == 0",
                        "Self::Live { lifecycle_ordinal, .. }",
                        "Self::Recovered { lifecycle_ordinal, .. }",
                        "Some(existing) if existing != ordinal",
                        "None => { *lifecycle_ordinal = Some(ordinal)",
                    ),
                    "single-assignment durable Validate retry lifecycle row binding",
                ),
                (
                    "release_lifecycle_ordinal",
                    (
                        "self.lifecycle_ordinal() != Some(ordinal)",
                        "Self::Live { lifecycle_ordinal, .. }",
                        "Self::Recovered { lifecycle_ordinal, .. }",
                        "*lifecycle_ordinal = None",
                    ),
                    "exact durable Validate retry lifecycle row release",
                ),
            ):
                ordinal_item = _require_qualified_rust_item(
                    effects_path,
                    effects_source,
                    "DurableValidateRetrySealV1",
                    ordinal_method,
                    errors,
                    description,
                )
                if ordinal_item is not None:
                    require_order(
                        effects_path,
                        description,
                        ordinal_item.source,
                        ordinal_fragments,
                    )
            cold_validate_retry_projection = _require_qualified_rust_item(
                effects_path,
                effects_source,
                "DurableValidateRetrySealV1",
                "project_retry",
                errors,
                "non-substitutable live and recovered Validate retry projection",
            )
            if cold_validate_retry_projection is not None:
                require_order(
                    effects_path,
                    "non-substitutable live and recovered Validate retry projection",
                    cold_validate_retry_projection.source,
                    (
                        "Self::Live { effect: incumbent_effect, ownership: incumbent_ownership, store_terminal, lifecycle_ordinal, }",
                        "store_terminal.as_ref().is_some_and(|store| !store.exactly_precedes_validate(effect))",
                        "adopt_incumbent_body_stage_for_retry_or_authority(incoming, effect)",
                        "seal: Self::Live",
                        "store_terminal: store_terminal.clone()",
                        "lifecycle_ordinal: *lifecycle_ordinal",
                        "Self::Recovered { owner, frontier, lifecycle_ordinal, }",
                        "owner.exactly_matches_retry(frontier, effect, incoming)",
                        "seal: Self::Recovered {",
                        "owner: Arc::clone(owner)",
                        "frontier",
                        "lifecycle_ordinal: *lifecycle_ordinal",
                    ),
                )
                reject_tokens(
                    effects_path,
                    "non-substitutable recovered Validate retry projection",
                    cold_validate_retry_projection.source,
                    ("owner: Arc::new(incoming",),
                )
            cold_validate_durable_commitment_join = _require_rust_item(
                effects_path,
                effects_source,
                "project_recovered_commitment_ceiling",
                errors,
            )
            if cold_validate_durable_commitment_join is not None:
                require_order(
                    effects_path,
                    "lineage-preserving recovered Validate durable commitment join",
                    cold_validate_durable_commitment_join.source,
                    (
                        "Self::Live { .. } => Ok(None)",
                        "Self::Recovered { owner, frontier, lifecycle_ordinal, }",
                        "frontier.project_commitment_ceiling(commitment)",
                        "owner: Arc::clone(owner)",
                        "frontier",
                        "lifecycle_ordinal: *lifecycle_ordinal",
                    ),
                )
            cold_validate_install = region(
                effects_path,
                effects_source,
                "atomic cold Ready Validate retry installation",
                "pub(in crate::sumeragi) struct PreparedRecoveredDurableValidateRetryInstallV1",
                "impl<R: EffectRuntime> V2EffectExecutor<R>",
            )
            require_tokens(
                effects_path,
                "atomic cold Ready Validate retry installation",
                cold_validate_install,
                (
                    "owner.exactly_matches_validated_marker(key, validated)",
                    "DurableValidateRetrySealV1::Recovered {",
                    "frontier: owner.initial_retry_frontier()",
                    "owner: Arc::new(owner)",
                ),
            )
            reject_tokens(
                effects_path,
                "atomic cold Ready Validate retry installation",
                cold_validate_install,
                ("DurableValidateRetrySealV1::Recovered(Arc::new(owner))",),
            )
            cold_validate_catalog_open = _require_rust_item(
                effects_path,
                effects_source,
                "open_with_body_store",
                errors,
            )
            if cold_validate_catalog_open is not None:
                require_order(
                    effects_path,
                    "owner-exact cold Ready Validate marker deferral",
                    cold_validate_catalog_open.source,
                    (
                        "body_store.ensure_recovered_markers_revalidated()",
                        "let recovered_bodies = body_store.recovery_catalog()",
                        "let recovered_validations = body_store.validated_recovery_catalog()",
                        "for (key, validated_receipt) in &recovered_validations",
                        "validated_receipt.durable() != durable_receipt",
                        "recovered_validate_retry_census.classify_and_bind_validated_marker(",
                        "runtime.recover_validated_body(manifest, validated_receipt)",
                        "Self::with_runtime_and_guard(",
                        "install_recovered_validation_catalog(",
                        "recovered_validate_retry_census.install_into_executor(",
                        "construction.complete()",
                    ),
                )
                require_token_count(
                    effects_path,
                    "owner-exact cold Ready Validate marker deferral",
                    cold_validate_catalog_open.source,
                    "Vec<RecoveredDurableValidateRetryOwnerV1>",
                    0,
                )
                require_token_count(
                    effects_path,
                    "owner-exact cold Ready Validate marker deferral",
                    cold_validate_catalog_open.source,
                    "recovered_validate_retry_census.iter(",
                    0,
                )
                require_token_count(
                    effects_path,
                    "owner-exact cold Ready Validate marker deferral",
                    cold_validate_catalog_open.source,
                    "mut recovered_validate_retry_census: RecoveredDurableValidateRetryCensusV1",
                    1,
                )
            cold_validate_record_marker = _require_rust_item(
                effects_path,
                effects_source,
                "record_lifecycle_validated_body",
                errors,
            )
            if cold_validate_record_marker is not None:
                require_order(
                    effects_path,
                    "pre-mutation recovered Validate marker commitment join",
                    cold_validate_record_marker.source,
                    (
                        "self.validated_bodies.get(&key).is_some_and(|existing| existing != &validated)",
                        "let projected_recovered_seal = self.durable_validate_retry_seals.get(&key)",
                        "seal.project_recovered_commitment_ceiling(validated.execution_commitment())",
                        ".transpose()",
                        ".map_err(EffectExecutorError::Contract)?",
                        "self.validated_bodies.entry(key).or_insert(validated)",
                        "self.durable_validate_retry_seals.insert(key, seal)",
                    ),
                )
            retain_effect_batch = _require_rust_item(
                effects_path,
                effects_source,
                "retain_effect_batch_at_frontier",
                errors,
            )
            if retain_effect_batch is not None:
                retry_start = retain_effect_batch.source.find(
                    "if let AdapterEffect::ValidateBody { round, subject, .. } = effect"
                )
                retry_end = retain_effect_batch.source.find(
                    "let mut candidate_semantic_identity = evidence.candidate_semantic_identity()",
                    retry_start,
                )
                if retry_start < 0 or retry_end < 0:
                    errors.append(
                        f"{effects_path}:{retain_effect_batch.line}: missing exact cold "
                        "Validate retry-stutter branch"
                    )
                else:
                    retry_stutter = retain_effect_batch.source[retry_start:retry_end]
                    require_order(
                        effects_path,
                        "exact cold Ready Validate retry stutter",
                        retry_stutter,
                        (
                            "retained_validate_retry_seals.get(&(*round, *subject)).cloned()",
                            "seal.project_retry(effect, evidence)",
                            "RuntimeCandidateAdmissionDisposition::CoalescedRetry",
                            "production_adapter_effect_candidate_trace_projection(",
                            "check_production_effect_to_candidate_transition(",
                            "retained_validate_retry_seals.insert((*round, *subject), projected.seal)",
                            "retain_effect.push(false)",
                            "continue",
                        ),
                    )
                    for forbidden in (
                        "install_pending_durable_validate_admission",
                        "enqueue_",
                        "dispatch_",
                    ):
                        require_token_count(
                            effects_path,
                            "exact cold Ready Validate retry stutter",
                            retry_stutter,
                            forbidden,
                            0,
                        )
            decision_cleanup = _require_rust_item(
                effects_path,
                effects_source,
                "reconcile_decision_work",
                errors,
            )
            if decision_cleanup is not None:
                require_order(
                    effects_path,
                    "Decision-scoped cold Validate retry cleanup",
                    decision_cleanup.source,
                    (
                        "let projected_recovered_decision_seal = self.durable_validate_retry_seals.get(&decision_body)",
                        "seal.project_recovered_commitment_ceiling(decision_commitment)",
                        ".transpose()",
                        ".map_err(EffectExecutorError::Contract)?",
                        "!self.pending_durable_validate_admissions.is_empty()",
                        "self.preflight_remote_proposal_replay_indexes()?",
                        "self.runtime.retire_proposal_work_after_decision(",
                        "let Some(seal) = projected_recovered_decision_seal",
                        "!drain_decision_body || seal.lifecycle_ordinal().is_some()",
                        "self.durable_validate_retry_seals.insert(decision_body, seal)",
                        "self.durable_validate_retry_seals.retain(|key, seal| { seal.lifecycle_ordinal().is_some() || (!drain_decision_body && *key == decision_body) })",
                        "self.protected_decision = Some(durable_decision)",
                    ),
                )
            lifecycle_launch_item = _require_qualified_rust_item(
                launch_path,
                launch_source,
                "ProductionLifecycleOwnerV1",
                "launch",
                errors,
                "cold Ready Validate owner launch installation",
                expected_attributes=(
                    "#[allow(clippy::result_large_err)]",
                    "#[inline(never)]",
                ),
            )
            if lifecycle_launch_item is not None:
                require_order(
                    launch_path,
                    "cold Ready Validate census launch installation",
                    lifecycle_launch_item.source,
                    (
                        "runtime.replayed_decision_key()",
                        "project_recovered_durable_validate_retry_census(",
                        "V2EffectExecutor::open_with_body_store(",
                        "recovered_validate_retry_census",
                        "ProductionV2Services::start_with_apply_service(",
                        "construction.complete()",
                    ),
                )
                require_token_count(
                    launch_path,
                    "cold Ready Validate census launch installation",
                    lifecycle_launch_item.source,
                    "&recovered_validate_retry_census",
                    0,
                )
                require_token_count(
                    launch_path,
                    "cold Ready Validate census launch installation",
                    lifecycle_launch_item.source,
                    "install_recovered_durable_validate_retry_owners",
                    0,
                )
                require_token_count(
                    launch_path,
                    "cold Ready Validate census launch installation",
                    lifecycle_launch_item.source,
                    "arm_live_clocks",
                    0,
                )
            lifecycle_activation = _require_rust_item(
                launch_path,
                launch_source,
                "activate_with",
                errors,
            )
            if lifecycle_activation is not None:
                require_order(
                    launch_path,
                    "post-install live clock and ingress activation",
                    lifecycle_activation.source,
                    (
                        "self.executor.arm_live_clocks(clock_activation, now)",
                        "self.services.activate_effect_completion_observer(observer)",
                        "publication.open_and_publish(&self.leader_wire_ingress_binding.ingress, status)",
                    ),
                )
            cold_apply_startup = _require_rust_item(
                ledger_path,
                ledger_source,
                "open_recovered_decision_apply_startup",
                errors,
            )
            if cold_apply_startup is not None:
                require_order(
                    ledger_path,
                    "cold recovered Decision Apply startup lineage",
                    cold_apply_startup.source,
                    (
                        "let (ledger_store, predecessor) = LifecycleLedgerStoreV1::open(",
                        "let fetch_is_present = predecessor",
                        "projection.fetch().names_record(record)",
                        "let staged_predecessor = if fetch_is_present",
                        "predecessor.clone()",
                        ".stage_authenticated_wal_decision_fetch(projection.fetch())",
                        "staged_predecessor .stage_recovered_decision_apply(projection.as_ref())",
                        "LifecycleCoordinator::prepare_with_authenticated_successor_store_borrowed(",
                        "authority, ledger_store, predecessor, successor.clone()",
                    ),
                )

            payload_terminal = _require_rust_item(
                schema_path,
                schema_source,
                "matches_terminal",
                errors,
            )
            if payload_terminal is not None:
                require_order(
                    schema_path,
                    "payload-free recovered Decision Fetch terminal identity",
                    payload_terminal.source,
                    (
                        "LifecycleWorkClass::Fetch, Self::None,",
                        "TerminalOutcome::Advanced",
                        "TerminalOutcome::Cancelled",
                        "TerminalOutcome::Rejected(_)",
                        "TerminalOutcome::Failed(_)",
                        "(LifecycleWorkClass::Fetch, Self::None, _) => false",
                    ),
                )
            restart_reconciliation = _require_rust_item(
                lifecycle_open_path,
                lifecycle_open_source,
                "reconcile_restart_inner",
                errors,
            )
            if restart_reconciliation is not None:
                require_order(
                    lifecycle_open_path,
                    "payload-free recovered Decision Fetch continuation",
                    restart_reconciliation.source,
                    (
                        "metadata.continuation.successor_parts()",
                        "recovered_decision_body_continuation_is_exact(",
                        ".or_else(|| { signed_broadcast_continuation_is_exact(",
                        ".unwrap_or_else(||",
                        "durable_continuation_payload_is_exact(",
                        "!payload_and_replay_are_exact",
                    ),
                )
            require_tokens(
                ledger_path,
                "payload-free signed-Broadcast restart regression",
                ledger_source,
                (
                    "fn all_sign_broadcast_continuations_roundtrip_with_canonical_wire_shapes()",
                    "exact_timeout_sign_broadcast_fixture(",
                    "durable_continuation_successor_is_exact(",
                    "signed_broadcast_continuation_is_exact(",
                    "Some(false)",
                    "parent.replay_authority_is_exact(context())",
                    "child.replay_authority_is_exact(context())",
                    "coordinator.reconcile_restart(RecoverySnapshot::new(",
                    "Some(super::super::CoordinatorFault::RecoveryRejected)",
                    "coordinator.records.is_empty()",
                ),
            )
            require_tokens(
                replay_authority_path,
                "recovered Decision body continuation regression",
                replay_authority_source,
                (
                    "fn recovered_decision_body_continuation_is_exact(",
                    "parent_payload == DurablePayloadReference::None",
                    "child_payload == DurablePayloadReference::BodyFrame(body_frame.durable_reference())",
                    "body_source.locator == fetch_locator",
                    "body_source.tag == fetch_tag",
                    "body_source.certificate == &fetch_certificate",
                    "fn recovered_decision_body_lineage_is_stage_closed_and_predecessor_bound()",
                ),
            )

            canonical_fragment = _require_rust_item(
                snapshot_path,
                snapshot_source,
                "canonical_json_fragment",
                errors,
            )
            if canonical_fragment is not None:
                require_order(
                    snapshot_path,
                    "canonical snapshot JSON fragment identity",
                    canonical_fragment.source,
                    (
                        "let value: json::Value = json::from_str(input)",
                        "json::to_json(&value)",
                    ),
                )
            scalar_hash = _require_rust_item(
                snapshot_path,
                snapshot_source,
                "update_snapshot_wsv_hash",
                errors,
            )
            if scalar_hash is not None:
                require_order(
                    snapshot_path,
                    "canonical snapshot scalar identity",
                    scalar_hash.source,
                    (
                        "Some(_) =>",
                        "let canonical = canonical_json_fragment(input)?",
                        "Digest::update(hasher, canonical.as_bytes())",
                    ),
                )
            object_hash = _require_rust_item(
                snapshot_path,
                snapshot_source,
                "update_snapshot_wsv_object_hash",
                errors,
            )
            if object_hash is not None:
                require_order(
                    snapshot_path,
                    "canonical staged snapshot event-buffer identity",
                    object_hash.source,
                    (
                        "path == CanonicalWsvPath::World",
                        "!members.iter().any(|member| member.key == \"external_event_buf\")",
                        "let Some(value) = overrides.committed_external_event_buf",
                        "members.push(BorrowedJsonMember",
                        "key: \"external_event_buf\".to_owned()",
                        "members.sort_unstable_by(",
                    ),
                )
                require_order(
                    snapshot_path,
                    "canonical snapshot object-key identity",
                    object_hash.source,
                    (
                        "let canonical_key = canonical_json_fragment(member.encoded_key)?",
                        "Digest::update(hasher, canonical_key.as_bytes())",
                    ),
                )
            string_set_hash = _require_rust_item(
                snapshot_path,
                snapshot_source,
                "update_sorted_string_set_hash",
                errors,
            )
            if string_set_hash is not None:
                require_order(
                    snapshot_path,
                    "canonical snapshot string-set identity",
                    string_set_hash.source,
                    (
                        "items.iter().any(|item| !item.starts_with('\"'))",
                        ".map(canonical_json_fragment)",
                        ".collect::<Result<Vec<_>, _>>()?",
                        "items.sort_unstable()",
                        "items.dedup()",
                    ),
                )
            require_tokens(
                snapshot_path,
                "canonical snapshot hash behavior",
                snapshot_source,
                (
                    "fn borrowed_snapshot_wsv_hash_canonicalizes_json_lexemes()",
                    "fn staged_snapshot_wsv_hash_injects_committed_event_buffer()",
                    "committed_external_event_buf: Some(committed_event_buffer)",
                    "Hash::new(canonical)",
                ),
            )
            apply_behavior = _require_rust_item(
                apply_path,
                apply_source,
                "validate_and_apply",
                errors,
            )
            if apply_behavior is not None:
                require_order(
                    apply_path,
                    "staged and committed snapshot hash parity",
                    apply_behavior.source,
                    (
                        "let staged_snapshot_bytes_for_test =",
                        "canonical_staged_state_snapshot_hash(&state_block)",
                        "staged_checkpoint, Hash::new(",
                        "store_wsv_checkpoint(context.height, block_hash, staged_checkpoint)",
                        "let committed = crate::snapshot::canonical_state_snapshot_bytes(self.state.as_ref())",
                        "crate::snapshot::canonical_state_snapshot_hash(self.state.as_ref())",
                        "Hash::new(&committed)",
                        "if staged != committed",
                    ),
                )

            all_live_census = _require_rust_item(
                registry_validate_path,
                registry_validate_source,
                "exactly_covers_all_live_work_with_optional_active_producer",
                errors,
            )
            if all_live_census is not None:
                require_tokens(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    all_live_census.source,
                    (
                        "coordinator.capacity_generation.keys().copied().collect::<std::collections::BTreeSet<_>>() != exact_capacity_classes",
                        "coordinator.admission_waits.len() > super::MAX_PENDING_ADMISSION_WAITS",
                        "candidate.replay_authority_is_exact(coordinator.active_context)",
                        "waiting.wait_token.observed_generation > coordinator.capacity_generation[&class]",
                        "LifecycleLedgerV1::from_coordinator(coordinator)",
                        "coordinator.episode_authority.universe_for(record.key).as_ref() != Some(&record.episode.universe)",
                        "wait.observed_generation == u64::MAX",
                        "coordinator.observed_generation.get(&wait.source).copied().unwrap_or(0) != wait.observed_generation",
                        "coordinator.owner_index != exact_owners",
                        "coordinator.ready_index != exact_ready",
                        "coordinator.capacity_used != exact_capacity_used",
                        "self.entries.len() != live.len()",
                        "serve_ordinal_pair_is_exact(serve, producer)",
                        "Arc::ptr_eq(&serve.replay_evidence, &producer.replay_evidence)",
                        "!paired_next_vote_addresses.is_subset(&exact_next_vote_addresses)",
                        "replay_authority == &metadata.replay_authority",
                        "sign.dispatch_key.is_none()",
                        "sign.repair.validates_in_ledger(&exact_ledger)",
                        "sign.carrier.validates_in_ledger(verified, &exact_ledger)",
                        "broadcast.validates_in_ledger(&exact_ledger)",
                        "fetch.carrier.validates_in_ledger(verified, &exact_ledger)",
                        "match (fetch.dispatch_key, fetch.wait_source)",
                        "(None, None)",
                        "(Some(key), Some(source))",
                        "key.matches(coordinator.active_context, address, digest)",
                        "fetch.matches_waiting_record( address, digest, coordinator, source, )",
                        "(None, Some(_)) | (Some(_), None) => false",
                        "store.fetch.validates(verified)",
                        "store.fetch.validates_recovered_store_in_ledger(&store.store, &exact_ledger)",
                        "apply.dispatch_key.is_none()",
                        "apply.carrier.validates_in_ledger(",
                        "verified, &exact_ledger, address.ordinal",
                    ),
                )
                require_token_count(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    all_live_census.source,
                    "coordinator.admission_waits.len() > super::MAX_PENDING_ADMISSION_WAITS",
                    1,
                )
                require_order(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    all_live_census.source,
                    (
                        "let exact_capacity_classes = CapacityClass::ALL",
                        "coordinator.admission_waits.iter().any",
                        "LifecycleLedgerV1::from_coordinator(coordinator)",
                        "coordinator.records.iter().any",
                        "coordinator.capacity_used != exact_capacity_used",
                        "let live = coordinator.records.iter()",
                        "self.entries.len() != live.len()",
                        "coordinator.producer_debts.iter().all",
                        "!paired_next_vote_addresses.is_subset(&exact_next_vote_addresses)",
                        "live.into_iter().all",
                    ),
                )
                require_token_count(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    all_live_census.source,
                    "sign.dispatch_key.is_none()",
                    4,
                )
                require_token_count(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    all_live_census.source,
                    "apply.dispatch_key.is_none()",
                    2,
                )
            fresh_serve_preflight = _require_rust_item(
                registry_path,
                registry_source,
                "preflights_fresh_registry",
                errors,
            )
            if fresh_serve_preflight is not None:
                require_order(
                    registry_path,
                    "fresh Serve exhaustive all-live registry census",
                    fresh_serve_preflight.source,
                    (
                        "self.preflights_registry(registry)",
                        "registry.exactly_covers_all_live_work(verified, current)",
                        "current.active_context == staged.active_context",
                        "self.exactly_matches_fresh_staged_append(current, staged)",
                    ),
                )
            fresh_staged_append = _require_rust_item(
                registry_path,
                registry_source,
                "exactly_matches_fresh_staged_append",
                errors,
            )
            if fresh_staged_append is not None:
                require_order(
                    registry_path,
                    "gap-aware fresh Serve staged append",
                    fresh_staged_append.source,
                    (
                        "let mut serve = None",
                        "let mut producer = None",
                        "for (address, work) in &self.entries",
                        "staged.records.get(&address.ordinal)",
                        "staged.durable_records.get(&address.ordinal)",
                        "ConcreteLifecycleWorkKind::DurableCertifiedServe(carrier)",
                        "serve.replace(address.ordinal).is_none()",
                        "carrier.matches_record(record, metadata, work.digest)",
                        "ConcreteLifecycleWorkKind::DurableProducerTurn(carrier)",
                        "producer.replace(address.ordinal).is_none()",
                        "carrier.matches_record(record, metadata, work.digest)",
                        "let (Some(serve), Some(producer)) = (serve, producer)",
                        "serve <= current.high_water",
                        "serve.checked_add(1) != Some(producer)",
                        "producer != staged.high_water",
                        "current.records.len().checked_add(2) != Some(staged.records.len())",
                        "current.durable_records.len().checked_add(2) != Some(staged.durable_records.len())",
                        "current.key_index.len().checked_add(2) != Some(staged.key_index.len())",
                        "current.owner_index.len().checked_add(1) != Some(staged.owner_index.len())",
                        "current.ready_index.len().checked_add(1) != Some(staged.ready_index.len())",
                        "current.producer_debts.len().checked_add(1) != Some(staged.producer_debts.len())",
                        "current.admission_waits != staged.admission_waits",
                        "current.active_lease != staged.active_lease",
                        "current.next_lease != staged.next_lease",
                        "current.capacity_geometry != staged.capacity_geometry",
                        "current.capacity_generation != staged.capacity_generation",
                        "current.observed_generation != staged.observed_generation",
                        "current.fault != staged.fault",
                        "current.records.iter().all",
                        "current.durable_records.iter().all",
                        "current.key_index.iter().all",
                        "current.owner_index.iter().all",
                        "current.ready_index.is_subset(&staged.ready_index)",
                        "current.producer_debts.iter().all",
                        "staged.producer_debts.get(&serve) != Some(&producer)",
                        "let mut expected_capacity = current.capacity_used.clone()",
                        "expected_capacity.get_mut(&CapacityClass::Serve)",
                        "serve_used.checked_add(1)",
                        "*serve_used = next_serve",
                        "expected_capacity.get_mut(&CapacityClass::Producer)",
                        "producer_used.checked_add(1)",
                        "*producer_used = next_producer",
                        "staged.capacity_used == expected_capacity",
                    ),
                )
            fresh_serve_install = _require_rust_item(
                registry_validate_path,
                registry_validate_source,
                "install_certified_serve_fresh_batch_before_publication",
                errors,
            )
            if fresh_serve_install is not None:
                require_order(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    fresh_serve_install.source,
                    (
                        "batch.preflights_fresh_registry(self, verified, current, staged)",
                        "return Err(CertifiedServeRegistryBatchPublicationError::Preflight(",
                        "batch",
                        "self.install_certified_serve_batch_before_publication(batch, publish)",
                    ),
                )
            fresh_serve_owner = _require_rust_item(
                lifecycle_projection_path,
                lifecycle_projection_source,
                "admit_selected_certified_serve",
                errors,
            )
            if fresh_serve_owner is not None:
                require_order(
                    lifecycle_projection_path,
                    "fresh Serve exhaustive all-live registry census",
                    fresh_serve_owner.source,
                    (
                        "!registry.exactly_covers_all_live_work(&self.verified, &self.coordinator)",
                        "retain_for_admission_with_verified_retention",
                        "let mut staged = self.coordinator.stage_durable_transaction()",
                        "staged.reduce_admit_with_durable_ordinals(AdmissionRequest::Candidate(candidate))",
                        "producer_turn_ordinal: Some(_)",
                        "let Some(ordinal_reservation) = ordinal_reservation",
                        "PreparedCertifiedServeRegistryBatchV1::from_fresh_admitted_pair",
                        "install_certified_serve_fresh_batch_before_publication",
                        "self.coordinator.persist_exact_staged_successor_with_ordinal_reservation( &staged, &ordinal_reservation, )",
                        "self.coordinator = staged",
                    ),
                )
            fresh_serve_ordinal_allocator = _require_rust_item(
                owner_path,
                owner_source,
                "reduce_admit_with_ordinal_allocator",
                errors,
            )
            if fresh_serve_ordinal_allocator is not None:
                require_order(
                    owner_path,
                    "fresh Serve and ProducerTurn reserve one exact two-ordinal range",
                    fresh_serve_ordinal_allocator.source,
                    (
                        "let producer = match (candidate.work_class, candidate.producer_turn.as_ref())",
                        "(LifecycleWorkClass::CertifiedServe, Some(producer)) => Some(producer)",
                        "let ordinal_count = if producer.is_some() { 2_usize } else { 1_usize }",
                        "allocate(self.high_water, ordinal_count)",
                    ),
                )
            durable_ordinal_admission = _require_rust_item(
                owner_path,
                owner_source,
                "reduce_admit_with_durable_ordinals",
                errors,
            )
            if durable_ordinal_admission is not None:
                require_order(
                    owner_path,
                    "live durable admission fences the allocator range",
                    durable_ordinal_admission.source,
                    (
                        "self.lifecycle_ordinal_authority.clone()",
                        "self.reduce_admit_with_ordinal_allocator",
                        "authority.begin_durable_range(high_water, count)",
                        "reservation = Some(pending)",
                    ),
                )
            durable_ordinal_publication = _require_rust_item(
                ledger_store_path,
                ledger_store_source,
                "persist_exact_staged_successor_with_ordinal_reservation",
                errors,
            )
            if durable_ordinal_publication is not None:
                require_order(
                    ledger_store_path,
                    "durable ordinal publication follows exact LedgerV1 fsync",
                    durable_ordinal_publication.source,
                    (
                        "self.persist_exact_staged_successor(staged)?",
                        "reservation.commit_after_durable_publication()",
                    ),
                )
            fresh_serve_pair_regression = _require_rust_item(
                owner_path,
                owner_source,
                "paired_launch_ordinal_authority_reserves_one_certified_serve_pair",
                errors,
            )
            if fresh_serve_pair_regression is not None:
                require_order(
                    owner_path,
                    "fresh Serve pair fences and publishes exactly two ordinals",
                    fresh_serve_pair_regression.source,
                    (
                        "ordinal: 14",
                        "producer_turn_ordinal: Some(15)",
                        "next_ordinal_for_test()",
                        "Some(14)",
                        "persist_durable_projection_with_ordinal_reservation(reservation.as_ref())",
                        "next_ordinal_for_test()",
                        "Some(16)",
                        "persisted.high_water(), 15",
                    ),
                )
            require_tokens(
                ledger_operations_path,
                "fresh Serve exhaustive all-live registry census",
                ledger_operations_source,
                (
                    "fn exactly_matches_recovered_decision_apply_carrier(",
                    "installed_apply_ordinal: u128",
                    "!changed && staged == *self && apply_ordinal == installed_apply_ordinal",
                ),
            )
            require_tokens(
                concrete_admission_path,
                "fresh Serve exhaustive all-live registry census regressions",
                concrete_admission_source,
                (
                    "fn exhaustive_live_registry_census_rejects_volatile_drift_and_one_missing_carrier()",
                    "WaitSource::Capacity(super::super::CapacityClass::Consensus)",
                    "WaitSource::Recovery(LifecycleDigest::new([0x33; 32]))",
                    "coordinator.observed_generation.insert(recovery_source, 1)",
                    "WaitToken::new(recovery_source, u64::MAX)",
                    ".capacity_generation.remove(&super::super::CapacityClass::Producer)",
                    ".episode.frozen_predecessors.insert(1)",
                    "remove_exact_for_test(address)",
                ),
            )
            recovered_broadcast_pair_fixture = _require_rust_item(
                registry_validate_path,
                registry_validate_source,
                "recovered_broadcast_pair_scheduler_fixture_for_test",
                errors,
            )
            if recovered_broadcast_pair_fixture is not None:
                require_order(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census regressions",
                    recovered_broadcast_pair_fixture.source,
                    (
                        "paired_next_sign",
                        "unrelated_sign",
                        "attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote",
                        "attest_ready_recovered_lifecycle_sign",
                        "exactly_covers_all_live_work(verified, coordinator)",
                    ),
                )
            require_tokens(
                ledger_path,
                "fresh Serve exhaustive all-live registry census regressions",
                ledger_source,
                (
                    "fn fresh_certified_serve_publishes_exact_ledger_beside_fetch_and_broadcast()",
                    "exactly_covers_all_live_work(&fixture.verified, &owner.coordinator)",
                    "owner.live_fetch_count_for_test()",
                ),
            )
            lifecycle_launch_item = _require_qualified_rust_item(
                launch_path,
                launch_source,
                "ProductionLifecycleOwnerV1",
                "launch",
                errors,
                "Kura-bound production lifecycle launch",
                expected_attributes=(
                    "#[allow(clippy::result_large_err)]",
                    "#[inline(never)]",
                ),
            )
            lifecycle_launch = (
                lifecycle_launch_item.source
                if lifecycle_launch_item is not None
                else ""
            )
            require_order(
                launch_path,
                "Kura-bound production lifecycle launch",
                lifecycle_launch,
                (
                    "begin_fail_stop_operation()",
                    "Self::launch_local_identity_matches( &context.roster, &inputs.local_peer, inputs.local_validator, &inputs.key_pair, )",
                    "binding.matches_launch_identity(inputs.kura.as_ref(), &inputs.key_pair)",
                    "service.matches_lifecycle_launch( &inputs.state, &inputs.kura, &context, &validator_set_pops, )",
                    "binding.storage_paths_for_launch(inputs.kura.as_ref())",
                    "prepare_leader_wire_launch(launch_storage.wal_path())",
                    "super::authority::lifecycle_ordinal_authorities_after_high_watermark(self.coordinator.high_water(),)",
                    "RuntimeLifecycleOrdinalSource::from_authority(runtime_ordinal_authority)",
                    "leader_wire_launch.restored_producer_ordinal_high_watermark()",
                    "lifecycle_ordinals.advance_past(high_watermark)",
                    "leader_wire_launch.open_gate(",
                    "leader_wire_restore.scheduler_ordinal_high_watermark()",
                    "self.coordinator.bind_live_lifecycle_ordinal_authority(coordinator_ordinal_authority)",
                    "ProductionLeaderWireIngressBindingV1::bind(",
                    "self.adapter_startup.take()",
                    "self.body_store.take()",
                    "self.apply_service.take()",
                    "V2EffectExecutor::open_with_body_store(",
                    "if let Some(authenticated_genesis) = inputs.authenticated_genesis.as_ref()",
                    "executor.install_authenticated_genesis_body(authenticated_genesis)",
                    "ProductionV2Services::start_with_apply_service(",
                    "ProductionLifecycleApplyServiceLaunchPermitV1",
                    "apply_service,",
                    "leader_wire_ingress_binding,",
                ),
            )
            runner_dependency_permit = region(
                runner_dependency_path,
                runner_dependency_source,
                "runner-sealed recovered lifecycle factory dependency permit",
                "pub(in crate::sumeragi) struct RecoveredLifecycleOwnerFactoryDependencyPermitV1",
                "/// Runner-private one-shot authority for activating a launched lifecycle height.",
            )
            require_tokens(
                runner_dependency_path,
                "runner-sealed recovered lifecycle factory dependencies",
                runner_dependency_permit,
                (
                    "struct RecoveredLifecycleOwnerFactoryDependencyPermitV1",
                    "_seal: RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
                    "local_signer: KeyPair",
                    "block_cadence: Duration",
                "fn mint_for_recovered_runner(local_signer: KeyPair, block_cadence: Duration,) -> Self",
                    "#[cfg(test)] pub(in crate::sumeragi) fn for_test(local_signer: KeyPair, block_cadence: Duration) -> Self",
                    "fn into_factory_dependencies(self) -> (KeyPair, Duration)",
                    "(self.local_signer, self.block_cadence)",
                    "impl Drop for RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
                ),
            )
            reject_tokens(
                runner_dependency_path,
                "runner-sealed recovered lifecycle factory dependencies",
                runner_dependency_permit,
                (
                    "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
                    "pub(crate) fn mint_for_recovered_runner(",
                    "pub fn mint_for_recovered_runner(",
                    "impl Clone for RecoveredLifecycleOwnerFactoryDependencyPermitV1",
                    "fn into_parts(",
                ),
            )
            lifecycle_activation = region(
                launch_path,
                launch_source,
                "one-shot lifecycle activation transaction",
                "fn activate_with(",
                "\n}\n\nimpl ActivatedProductionLifecycleV1",
            )
            require_order(
                launch_path,
                "one-shot lifecycle activation transaction",
                lifecycle_activation,
                (
                    "begin_fail_stop_operation()",
                    "self.executor.local_proposal_directive()",
                    "local_proposal.exactly_matches( self.executor.context().id(), current_directive )",
                    "ProductionLifecycleActivationErrorV1::LocalProposalPreparationMismatch",
                    "let clock_activation = ProductionLifecycleLiveClockActivationPermitV1",
                    "self.executor.arm_live_clocks(clock_activation, now)",
                    "self.executor.successor_activation_status_snapshot()",
                    "self.completion_observer_activation.take()",
                    "self.services.activate_effect_completion_observer(observer)",
                    "publication.open_and_publish( &self.leader_wire_ingress_binding.ingress, status )?",
                    "activation.complete()",
                    "ActivatedProductionLifecycleV1 { runner_activation, local_proposal, launched: self, }",
                ),
            )
            reject_tokens(
                launch_path,
                "one-shot lifecycle activation transaction",
                lifecycle_activation,
                (
                    "set_v2_status",
                    "into_parts",
                    "into_owner",
                    "into_executor",
                    "into_services",
                ),
            )
            activated_owner = region(
                launch_path,
                launch_source,
                "opaque activated lifecycle owner",
                "struct ActivatedProductionLifecycleV1",
                "enum ProductionLifecycleActivationPublicationV1",
            )
            require_tokens(
                launch_path,
                "opaque activated lifecycle owner",
                activated_owner,
                (
                    "runner_activation: super::super::v2_runner::ProductionLifecycleActivatedRunnerAuthorityV1",
                    "local_proposal: ProductionLifecyclePreparedLocalProposalStateV1",
                    "launched: LaunchedProductionLifecycleV1",
                ),
            )
            require_order(
                launch_path,
                "opaque activated lifecycle owner drop order",
                activated_owner,
                (
                    "runner_activation: super::super::v2_runner::ProductionLifecycleActivatedRunnerAuthorityV1",
                    "local_proposal: ProductionLifecyclePreparedLocalProposalStateV1",
                    "launched: LaunchedProductionLifecycleV1",
                ),
            )
            reject_tokens(
                launch_path,
                "opaque activated lifecycle owner",
                activated_owner,
                (
                    "pub launched:",
                    "pub(crate) launched:",
                    "pub(in crate::sumeragi) launched:",
                    "pub runner_activation:",
                    "pub(crate) runner_activation:",
                    "pub(in crate::sumeragi) runner_activation:",
                    "pub local_proposal:",
                    "pub(crate) local_proposal:",
                    "pub(in crate::sumeragi) local_proposal:",
                    "impl Clone for ActivatedProductionLifecycleV1",
                    "impl Copy for ActivatedProductionLifecycleV1",
                ),
            )
            activated_runner_borrow = region(
                launch_path,
                launch_source,
                "borrow-bound activated lifecycle owner",
                "fn with_runner_runtime<R>(",
                "impl FinalizedProductionLifecycleRolloverV1",
            )
            require_tokens(
                launch_path,
                "borrow-bound activated lifecycle owner",
                activated_runner_borrow,
                (
                    "fn with_runner_runtime<R>(",
                    "_runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1",
                    "&mut super::super::v2_runner::ProductionLifecycleLocalProposalStateV1",
                    ".prepared_local_proposal_mut()",
                    "&mut self.launched.owner",
                    "&mut self.launched.executor",
                    "&mut self.launched.services",
                    "local_proposal",
                ),
            )
            reject_tokens(
                launch_path,
                "borrow-bound activated lifecycle owner",
                activated_runner_borrow,
                (
                    "into_parts",
                    "into_owner",
                    "into_executor",
                    "into_services",
                    "pub launched:",
                    "pub(crate) launched:",
                ),
            )
            ordinary_runner_activation = region(
                runner_dependency_path,
                runner_dependency_source,
                "runner-owned lifecycle activation authority",
                "struct ProductionLifecycleRunnerActivationV1",
                "struct ProductionLifecycleCompleteTipRunnerActivationV1",
            )
            require_order(
                runner_dependency_path,
                "runner-owned lifecycle activation authority",
                ordinary_runner_activation,
                (
                    "self.ingress_ready.store(false, Ordering::Release)",
                    "Arc::ptr_eq(&self.block_ingress, launched_ingress)",
                    "self.block_ingress.close()",
                    "self.block_ingress.open()",
                    "let publication = match self.status",
                    "self.block_ingress.close()",
                    "self.ingress_ready.store(true, Ordering::Release)",
                ),
            )
            require_tokens(
                runner_dependency_path,
                "runner-owned lifecycle activation status classes",
                ordinary_runner_activation,
                (
                    "_seal: ProductionLifecycleRunnerActivationSealV1",
                    "struct ProductionLifecycleRunnerActivationSealV1",
                    "impl Drop for ProductionLifecycleRunnerActivationSealV1",
                    "fn current_height(",
                    "fn applied(",
                    "fn snapshot_bootstrap(",
                    "status: ProductionLifecycleRunnerStatusAuthorityV1",
                    "CurrentHeight",
                    "Applied",
                    "SnapshotBootstrap",
                    "status::set_v2_status(successor)",
                    "status::activate_v2_successor_height(",
                    "status::activate_snapshot_bootstrap_v2_height(",
                    "ProductionLifecycleActivatedRunnerAuthorityV1 { _seal: ProductionLifecycleActivatedRunnerAuthoritySealV1, ingress_ready: self.ingress_ready, block_ingress: self.block_ingress, }",
                ),
            )
            reject_tokens(
                runner_dependency_path,
                "runner-owned lifecycle activation status classes",
                ordinary_runner_activation,
                (
                    "impl Clone for ProductionLifecycleRunnerActivationV1",
                    "impl Copy for ProductionLifecycleRunnerActivationV1",
                    "pub(in crate::sumeragi) fn current_height(",
                    "pub(crate) fn current_height(",
                    "pub fn current_height(",
                    "pub(in crate::sumeragi) fn applied(",
                    "pub(in crate::sumeragi) fn snapshot_bootstrap(",
                    "fn into_parts(",
                ),
            )
            complete_tip_runner_activation = region(
                runner_dependency_path,
                runner_dependency_source,
                "runner-owned CompleteTip lifecycle activation authority",
                "struct ProductionLifecycleCompleteTipRunnerActivationV1",
                "struct ProductionLifecycleActivatedRunnerAuthorityV1",
            )
            require_order(
                runner_dependency_path,
                "runner-owned CompleteTip lifecycle activation authority",
                complete_tip_runner_activation,
                (
                    "self.ingress_ready.store(false, Ordering::Release)",
                    "Arc::ptr_eq(&self.block_ingress, launched_ingress)",
                    "self.block_ingress.close()",
                    "retirement.authorizes_successor_status(&successor)",
                    "self.block_ingress.close()",
                    "self.block_ingress.open()",
                    "status::activate_recovered_complete_tip_v2_height(retirement, successor)",
                    "self.block_ingress.close()",
                    "self.ingress_ready.store(true, Ordering::Release)",
                ),
            )
            require_tokens(
                runner_dependency_path,
                "runner-owned CompleteTip lifecycle activation seal",
                complete_tip_runner_activation,
                (
                    "_seal: ProductionLifecycleCompleteTipRunnerActivationSealV1",
                    "struct ProductionLifecycleCompleteTipRunnerActivationSealV1",
                    "impl Drop for ProductionLifecycleCompleteTipRunnerActivationSealV1",
                    "fn mint_for_recovered_runner(",
                    "ProductionLifecycleActivatedRunnerAuthorityV1 { _seal: ProductionLifecycleActivatedRunnerAuthoritySealV1, ingress_ready: self.ingress_ready, block_ingress: self.block_ingress, }",
                ),
            )
            reject_tokens(
                runner_dependency_path,
                "runner-owned CompleteTip lifecycle activation seal",
                complete_tip_runner_activation,
                (
                    "impl Clone for ProductionLifecycleCompleteTipRunnerActivationV1",
                    "impl Copy for ProductionLifecycleCompleteTipRunnerActivationV1",
                    "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
                    "pub(crate) fn mint_for_recovered_runner(",
                    "pub fn mint_for_recovered_runner(",
                    "fn into_parts(",
                ),
            )
            activated_runner_authority = region(
                runner_dependency_path,
                runner_dependency_source,
                "activated runner readiness and ingress authority",
                "struct ProductionLifecycleActivatedRunnerAuthorityV1",
                "struct ProductionLifecycleActiveRunnerBorrowV1",
            )
            require_tokens(
                runner_dependency_path,
                "activated runner readiness and ingress authority",
                activated_runner_authority,
                (
                    "_seal: ProductionLifecycleActivatedRunnerAuthoritySealV1",
                    "ingress_ready: Arc<AtomicBool>",
                    "block_ingress: Arc<FairV2Ingress>",
                    "impl Drop for ProductionLifecycleActivatedRunnerAuthoritySealV1",
                    "fn retire(",
                    "self.ingress_ready.store(false, Ordering::Release)",
                    "self.block_ingress.close()",
                    "retire_lifecycle_runner_ingress(&self.ingress_ready, &self.block_ingress, launched_ingress)",
                    "impl Drop for ProductionLifecycleActivatedRunnerAuthorityV1",
                ),
            )
            reject_tokens(
                runner_dependency_path,
                "activated runner readiness and ingress authority",
                activated_runner_authority,
                (
                    "impl Clone for ProductionLifecycleActivatedRunnerAuthorityV1",
                    "impl Copy for ProductionLifecycleActivatedRunnerAuthorityV1",
                    "fn into_parts(",
                    "pub ingress_ready:",
                    "pub block_ingress:",
                ),
            )
            require_token_count(
                runner_dependency_path,
                "activated runner readiness retirement",
                activated_runner_authority,
                "self.ingress_ready.store(false, Ordering::Release)",
                2,
            )
            require_token_count(
                runner_dependency_path,
                "activated runner ingress retirement",
                activated_runner_authority,
                "self.block_ingress.close()",
                2,
            )
            activated_runner_close = _require_qualified_rust_item(
                runner_dependency_path,
                runner_dependency_source,
                "ProductionLifecycleActivatedRunnerAuthorityV1",
                "close_ingress",
                errors,
                "activated runner finite-drain ingress closure",
            )
            if activated_runner_close is not None:
                require_order(
                    runner_dependency_path,
                    "activated runner finite-drain ingress closure",
                    activated_runner_close.source,
                    (
                        "self.ingress_ready.store(false, Ordering::Release)",
                        "self.block_ingress.close()",
                        "Arc::ptr_eq(&self.block_ingress, launched_ingress)",
                    ),
                )
            shared_runner_ingress_retirement = region(
                runner_dependency_path,
                runner_dependency_source,
                "shared lifecycle runner ingress retirement",
                "fn retire_lifecycle_runner_ingress(",
                "impl Drop for ProductionLifecycleActivatedRunnerAuthorityV1",
            )
            require_order(
                runner_dependency_path,
                "shared lifecycle runner ingress retirement",
                shared_runner_ingress_retirement,
                (
                    "ingress_ready.store(false, Ordering::Release)",
                    "block_ingress.close()",
                    "Arc::ptr_eq(block_ingress, launched_ingress)",
                ),
            )
            active_runner_borrow = region(
                runner_dependency_path,
                runner_dependency_source,
                "runner-owned active lifecycle borrow key",
                "struct ProductionLifecycleActiveRunnerBorrowV1",
                "/// Process-local borrow key for preparing a launched lifecycle before activation.",
            )
            require_tokens(
                runner_dependency_path,
                "runner-owned active lifecycle borrow key",
                active_runner_borrow,
                (
                    "_seal: ProductionLifecycleActiveRunnerBorrowSealV1",
                    "fn mint_for_recovered_runner() -> Self",
                    "impl Drop for ProductionLifecycleActiveRunnerBorrowSealV1",
                ),
            )
            reject_tokens(
                runner_dependency_path,
                "runner-owned active lifecycle borrow key",
                active_runner_borrow,
                (
                    "pub(in crate::sumeragi) fn mint_for_recovered_runner",
                    "pub(crate) fn mint_for_recovered_runner",
                    "pub fn mint_for_recovered_runner",
                    "fn into_parts(",
                    "impl Clone for ProductionLifecycleActiveRunnerBorrowV1",
                ),
            )
            require_tokens(
                launch_path,
                "local launch identity preflight",
                launch_source,
                (
                    "fn launch_local_identity_matches(",
                    "local_peer.public_key() != key_pair.public_key()",
                    "local_validator.is_none_or(|observed| roster_position == Some(observed))",
                    "fn launch_local_identity_requires_the_bound_key_and_exact_roster_position()",
                ),
            )
            require_tokens(
                launch_path, "single restored lifecycle ordinal source", lifecycle_launch,
                (
                    "inputs.auxiliary_io_capacity",
                    "lifecycle_ordinals.clone()", "lifecycle_ordinals .advance_past(leader_wire_restore.scheduler_ordinal_high_watermark())",
                ),
            )
            require_token_count(
                launch_path, "single restored lifecycle ordinal source",
                lifecycle_launch, "lifecycle_ordinals.clone()", 2,
            )
            require_token_count(
                launch_path,
                "certified Serve restore/start capacity parity",
                lifecycle_launch,
                "inputs.auxiliary_io_capacity",
                1,
            )
            require_tokens(
                launch_path,
                "move-only authenticated genesis launch input",
                launch_source,
                (
                    "authenticated_genesis: Option<AuthenticatedGenesisBodyV1>",
                    "if let Some(authenticated_genesis) = inputs.authenticated_genesis.as_ref()",
                    "executor.install_authenticated_genesis_body(authenticated_genesis)",
                ),
            )
            reject_tokens(
                launch_path,
                "move-only authenticated genesis launch input",
                region(
                    launch_path,
                    launch_source,
                    "sealed production launch inputs",
                    "pub(in crate::sumeragi) struct ProductionLifecycleLaunchInputsV1 {",
                    "\n}",
                ),
                (
                    "authenticated_genesis: Option<SignedBlock>",
                    "genesis_account: AccountId",
                    "chunk_root: PathBuf",
                    "wal_path: PathBuf",
                    "lifecycle_ordinals: RuntimeLifecycleOrdinalSource",
                    "durable_bodies:",
                    "recovered_body_receipts:",
                    "queue: Arc<Queue>",
                    "provider_ingest_finalized_archive:",
                    "reputation_finalized_archive:",
                    "block_cadence: Duration",
                    "events_sender: EventsSender",
                ),
            )
            require_tokens(
                worker_path,
                "sealed replay-service worker transfer",
                worker_source,
                (
                    "fn start_with_apply_service(",
                    "_permit: super::v2_lifecycle_coordinator::ProductionLifecycleApplyServiceLaunchPermitV1",
                    "apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops)",
                    "Self::start_inner(",
                ),
            )
            legacy_worker_start = region(
                worker_path,
                worker_source,
                "legacy worker Apply-service construction",
                "pub(crate) fn start(",
                "pub(in crate::sumeragi) fn start_with_apply_service(",
            )
            require_order(
                worker_path,
                "legacy worker Apply-service construction",
                legacy_worker_start,
                (
                    "let apply_service = V2ApplyService::new(",
                    "Self::start_inner(",
                ),
            )
            reject_tokens(
                worker_path,
                "legacy worker Apply-service construction",
                legacy_worker_start,
                ("Self::start_with_apply_service(",),
            )
            require_token_count(
                worker_path,
                "sealed replay-service worker transfer",
                worker_source,
                "ProductionLifecycleApplyServiceLaunchPermitV1",
                1,
            )
            require_token_count(
                launch_path,
                "sealed replay-service permit mint",
                launch_source,
                "ProductionLifecycleApplyServiceLaunchPermitV1 {",
                1,
            )
            require_tokens(
                state_path,
                "fixed State/Kura identity oracle",
                state_source,
                (
                    "fn matches_kura_instance(&self, kura: &Arc<Kura>) -> bool",
                    "Arc::ptr_eq(&self.kura, kura)",
                ),
            )
            require_tokens(
                apply_path,
                "fixed recovered Apply-service identity oracle",
                apply_source,
                (
                    "fn matches_lifecycle_launch(",
                    "Arc::ptr_eq(&self.state, state)",
                    "Arc::ptr_eq(&self.kura, kura)",
                    "self.network_id == context.network_id",
                    "self.validator_set_pops == validator_set_pops",
                ),
            )
            require_tokens(
                launch_path,
                "sealed leader-wire launch binding",
                launch_source,
                (
                    "struct ProductionLeaderWireIngressBindingV1",
                    "gate: Option<Arc<LeaderWireLifecycleStoreGate>>",
                    "fn bind(",
                    "ingress.bind_leader_wire_lifecycle_gate(",
                    "fn retire(&mut self)",
                    "self.gate.as_ref().cloned()",
                    "self.ingress.retire_leader_wire_lifecycle_gate(&gate)",
                    "self.gate = None",
                    "impl Drop for ProductionLeaderWireIngressBindingV1",
                    "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
                ),
            )
            require_tokens(
                adapter_path,
                "sealed adapter leader-wire launch projection",
                adapter_source,
                (
                    "struct ProductionLeaderWireLaunchAuthorityV1",
                    "fn prepare_leader_wire_launch(",
                    "adapter.wal.matches_path(expected_wal_path)",
                    "leader_wire_launch_prepared: false",
                    "!*leader_wire_launch_prepared",
                    "*leader_wire_launch_prepared = true",
                    "fn open_gate(",
                    "body_store: &super::v2_body_store::V2BodyStore",
                    "body_store.matches_context(context)",
                    "body_store.recovery_catalog()",
                    "LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(",
                ),
            )
            require_tokens(
                safety_wal_path,
                "opened safety-WAL directory authority",
                safety_wal_source,
                (
                    "struct SafetyWalServicedCandidateStoreAuthority",
                    "struct SafetyWalLeaderWireStoreAuthority",
                    "direct_lexical_directory_metadata(expected_path)",
                    "open_canonical_directory_nofollow(&canonical_path)",
                    "fn mint_serviced_candidate_store_authority(",
                    "fn mint_leader_wire_store_authority(",
                    "fn publish_atomic(&self, frame: &[u8], maximum: u64",
                    "let durable = rustix::fs::statat(",
                    "fn write_all(&mut self, bytes: &[u8])",
                    "fn sync_data(&mut self)",
                    "BoundSafetyWalDirectory::from_kura_authority(kura, authority)",
                ),
            )
            require_literal_count(
                safety_wal_path,
                "opened safety-WAL exact Kura identity rejection",
                safety_wal_source,
                '"safety-WAL authority belongs to a different Kura instance"',
                1,
            )
            require_tokens(
                kura_path,
                "Kura-root safety-WAL authority",
                kura_source,
                (
                    "struct KuraSafetyWalDirectoryAuthority",
                    "fn mint_safety_wal_directory_authority(",
                    "rustix::fs::openat(&root.file, STORE_ROOT_LOCK_FILE_NAME",
                    "Self::sidecar_file_metadata_unchanged(&lock_before, &linked_metadata)",
                    "rustix::fs::mkdirat(&parent.file, name, rustix::fs::Mode::RWXU)",
                    "Self::open_bound_progress_child_directory(",
                    "kura_identity: self.instance_identity()",
                ),
            )
            reject_tokens(
                safety_wal_path,
                "move-only safety-WAL sibling authorities",
                safety_wal_source,
                (
                    "impl Clone for SafetyWalServicedCandidateStoreAuthority",
                    "impl Clone for SafetyWalLeaderWireStoreAuthority",
                    "impl Copy for SafetyWalServicedCandidateStoreAuthority",
                    "impl Copy for SafetyWalLeaderWireStoreAuthority",
                ),
            )
            require_tokens(
                adjacent_store_path,
                "typed WAL-adjacent production stores",
                adjacent_store_source,
                (
                    "storage: SafetyWalServicedCandidateStoreAuthority",
                    "storage: SafetyWalLeaderWireStoreAuthority",
                    "fn open_with_safety_wal_authority(",
                    "self.storage.read_bounded(self.max_frame_bytes)",
                    "self.storage.publish_atomic(&frame, self.max_frame_bytes)",
                ),
            )
            serviced_candidate_open = _require_qualified_rust_item(
                adjacent_store_path,
                adjacent_store_source,
                "ServicedCandidateStore",
                "open_with_safety_wal_authority",
                errors,
                "typed WAL-adjacent production stores omits production refinement tokens in the serviced-candidate constructor",
            )
            _require_rust_token_sequence(
                adjacent_store_path,
                serviced_candidate_open,
                "storage: SafetyWalServicedCandidateStoreAuthority",
                "typed WAL-adjacent production stores omits production refinement tokens in the serviced-candidate constructor",
                errors,
            )
            leader_wire_open = _require_qualified_rust_item(
                adjacent_store_path,
                adjacent_store_source,
                "LeaderWireLifecycleStoreGate",
                "open_with_safety_wal_authority",
                errors,
                "typed WAL-adjacent production stores omits production refinement tokens in the leader-wire constructor",
                expected_attributes=("#[allow(clippy::too_many_arguments)]",),
            )
            _require_rust_token_sequence(
                adjacent_store_path,
                leader_wire_open,
                "storage: SafetyWalLeaderWireStoreAuthority",
                "typed WAL-adjacent production stores omits production refinement tokens in the leader-wire constructor",
                errors,
            )
            reject_tokens(
                adapter_path,
                "move-only leader-wire launch authority",
                adapter_source,
                (
                    "impl Clone for ProductionLeaderWireLaunchAuthorityV1",
                    "impl Clone for RecoveredLifecycleStorageAuthorityV1",
                    "impl Clone for RecoveredLifecycleLaunchStoragePathsV1",
                ),
            )
            require_tokens(
                owner_path,
                "production lifecycle owner Kura seal",
                owner_source,
                (
                    "kura_binding: Option<crate::sumeragi::v2::RecoveredLifecycleOwnerKuraBindingV1>",
                    "apply_service: Option<crate::sumeragi::v2_apply::V2ApplyService>",
                    "fn with_recovered_kura_binding_and_apply_service(",
                    "assert!(self.kura_binding.is_none())",
                    "assert!(self.apply_service.is_none())",
                    "self.kura_binding = Some(binding)",
                    "self.apply_service = Some(apply_service)",
                    "struct ProductionLifecycleApplyServiceLaunchPermitV1",
                    "impl Drop for ProductionLifecycleApplyServiceLaunchPermitSealV1",
                ),
            )
            recovered_sign_dispatch = region(
                scheduler_path,
                scheduler_source,
                "lifecycle-owned recovered Sign dispatch",
                "fn dispatch_recovered_lifecycle_sign_with_runner_debt(",
                "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
            )
            require_order(
                scheduler_path,
                "lifecycle-owned recovered Sign dispatch",
                recovered_sign_dispatch,
                (
                    "let Some(body_store_identity) = self.body_store_identity.as_ref()",
                    "services.matches_lifecycle_body_store(body_store_identity)",
                    "services.matches_lifecycle_executor_output_guard(executor)",
                    "attest_ready_recovered_lifecycle_sign",
                    "capture_recovered_lifecycle_sign_capacity(dispatch_key)",
                    "self.coordinator.plan_turn(inputs)",
                    "reservation.class() == CapacityClass::Consensus",
                    "prepare_recovered_lifecycle_sign_dispatch",
                    "reservation.preflight(&prepared)",
                    "reservation.commit(prepared)",
                ),
            )
            require_token_count(
                scheduler_path,
                "recovered Sign post-claim rollback",
                recovered_sign_dispatch,
                "self.coordinator.rollback_unpublished_turn(&lease)",
                1,
            )
            require_token_count(
                scheduler_path,
                "recovered Sign reserved post-claim rollback",
                recovered_sign_dispatch,
                "rollback_unpublished_reserved_turn(&lease",
                3,
            )
            require_token_count(
                scheduler_path,
                "recovered Sign reservation release",
                recovered_sign_dispatch,
                "reservation.cancel_uncommitted()",
                6,
            )
            reject_tokens(
                scheduler_path,
                "sealed recovered Sign dispatch",
                recovered_sign_dispatch,
                (
                    "AdapterEffect",
                    "PendingRuntimeEffectBinding",
                    "RuntimeEffectOwnership",
                    "EffectWorkId",
                    "into_parts",
                ),
            )
            recovered_phase_sign = region(
                registry_path,
                registry_source,
                "current-parent-bound recovered PhaseVote carrier",
                "impl DurableRecoveredWalSignWork {",
                "/// Whether one concrete registry row is still an executable adapter effect",
            )
            require_token_count(
                registry_path,
                "current-parent-bound recovered PhaseVote carrier",
                recovered_phase_sign,
                "self.matches_current_terminal_parent(coordinator)",
                2,
            )
            require_token_count(
                registry_path,
                "standalone recovered PhaseVote child",
                recovered_phase_sign,
                "metadata.continuation == super::schema::DurableContinuation::None",
                2,
            )
            require_tokens(
                registry_path,
                "current terminal Validate parent rejoin",
                recovered_phase_sign,
                (
                    "record.state == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)",
                    "metadata.matches_admission(parent)",
                    "super::schema::DurableContinuation::successor(",
                    "coordinator.key_index.get(&parent.key)",
                    "coordinator.owner_index.get(&parent.causal_root)",
                ),
            )
            recovered_sign_identity = region(
                registry_path,
                registry_source,
                "complete recovered Sign effect identity",
                "impl RecoveredLifecycleSignDispatchIdentityV1 {",
                "/// Read-only coordinates of one exact Waiting Fetch incumbent.",
            )
            require_tokens(
                registry_path,
                "complete recovered Sign effect identity",
                recovered_sign_identity,
                (
                    "&AdapterEffect::Sign {",
                    "request: request.clone()",
                    "adapter_effect_matches_lifecycle_digest(",
                ),
            )
            reject_tokens(
                registry_path,
                "historical recovered Commit identity",
                recovered_sign_identity,
                ("tag.view() ==", "vote.round.view"),
            )
            recovered_sign_task = region(
                worker_path,
                worker_source,
                "opaque recovered Sign worker task/result",
                "pub(in crate::sumeragi) struct RecoveredLifecycleSignTaskV1 {",
                "enum V2IoCommand {",
            )
            require_tokens(
                worker_path,
                "opaque recovered Sign worker task/result",
                recovered_sign_task,
                (
                    "identity: RecoveredLifecycleSignDispatchIdentityV1",
                    "prepared_candidate: Option<PreparedCandidateBody>",
                    "self.task.prepared_candidate == expected_prepared",
                    "outbound_payload: Option<EncodedV2Payload>",
                    "authorizes_request(self.task.tag, &self.task.request)",
                ),
            )
            reject_tokens(
                worker_path,
                "opaque recovered Sign worker task/result",
                recovered_sign_task,
                (
                    "pub tag:",
                    "pub request:",
                    "pub signature:",
                    "pub outbound_payload:",
                    "fn into_parts(",
                    "fn into_result(",
                    "fn into_task(",
                    "fn request(",
                    "fn prepared_candidate(",
                    "fn result(",
                    "fn acknowledgement(",
                    "fn acknowledge(",
                    "fn signature(",
                    "fn outbound_payload(",
                ),
            )
            parked_sign_completion = region(
                worker_path,
                worker_source,
                "parked recovered Sign completion",
                "pub(in crate::sumeragi) struct PreparedRecoveredLifecycleSignCompletionV1 {",
                "/// Result of atomically returning one guarded missing-sidecar Apply",
            )
            reject_tokens(
                worker_path,
                "parked recovered Sign completion",
                parked_sign_completion,
                (
                    "fn into_parts(",
                    "fn into_result(",
                    "fn into_task(",
                    "fn request(",
                    "fn prepared_candidate(",
                    "fn result(",
                    "fn acknowledgement(",
                    "fn acknowledge(",
                    "fn signature(",
                    "fn outbound_payload(",
                    "fn settle(",
                ),
            )
            require_tokens(
                worker_path,
                "adapter-private recovered Sign completion projection",
                parked_sign_completion,
                (
                    "fn project_adapter_completion_authority(",
                    "result.is_exact()",
                    "RecoveredLifecycleSignAdapterCompletionAuthorityV1 {",
                ),
            )
            require_tokens(
                worker_path,
                "post-publication recovered Sign completion acknowledgement",
                parked_sign_completion,
                (
                    "fn acknowledge_after_publication(self)",
                    "self.queue.acknowledge_recovered_lifecycle_sign(key)",
                    "self.guarded.acknowledge_after_publication()",
                ),
            )
            recovered_sign_preview = region(
                adapter_path,
                adapter_source,
                "drop-inert recovered Sign adapter preview",
                "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion(",
                "pub(crate) fn application_completed(",
            )
            require_order(
                adapter_path,
                "drop-inert recovered Sign adapter preview",
                recovered_sign_preview,
                (
                    "authority.consume_for_adapter(RecoveredLifecycleSignAdapterCompletionPermitV1::new())",
                    "verify_individual_signature(",
                    "let mut next_reducer = self.reducer.clone()",
                    "let outcome = next_reducer.step(event.clone())",
                    "if converted.first() != Some(&expected_broadcast)",
                    "Ok(PreparedRecoveredLifecycleSignAdapterCompletionV1 {",
                ),
            )
            require_tokens(
                adapter_path,
                "closed recovered Sign adapter successor shapes",
                recovered_sign_preview,
                (
                    "SignRequest::Proposal(_), Some((persist_tag, entry)), None",
                    "SignRequest::Proposal(_), None, Some(AdapterEffect::Sign { request: SignRequest::Vote(vote), .. })",
                    "vote.phase == wire::GlobalPhase::Prepare",
                    "SignRequest::Vote(_) | SignRequest::TimeoutVote(_), None, possible_next_sign",
                    "next_reducer.pending_persistence_record().is_none()",
                    "next_reducer.awaiting_signature()",
                    "RecoveredLifecycleSignCompletionMismatch",
                ),
            )
            reject_tokens(
                adapter_path,
                "drop-inert recovered Sign adapter preview",
                recovered_sign_preview,
                (
                    "self.wal.append(",
                    "self.reducer =",
                    "self.registry =",
                    "publish_effect",
                    "send(",
                ),
            )
            require_tokens(
                adapter_path,
                "recovered Sign adapter preview behavior regression",
                adapter_source,
                (
                    "fn recovered_timeout_signature_preview_is_exact_and_drop_inert()",
                    "fn production_recovered_proposal_sign_joins_exact_next_vote_body_store()",
                    "output.prepare_wal_append_permit().is_none()",
                ),
            )
            next_vote_service_join = region(
                worker_path,
                worker_source,
                "single-preview recovered next-Vote body service join",
                "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion_with_body<'executor>(",
                "pub(in crate::sumeragi) fn activate_effect_completion_observer(",
            )
            require_order(
                worker_path,
                "single-preview recovered next-Vote body service join",
                next_vote_service_join,
                (
                    "self.recovered_lifecycle_next_vote_body_executor_permit(executor)?",
                    "executor.prepare_recovered_lifecycle_sign_completion_with_body(permit, completion)",
                ),
            )
            reject_tokens(
                worker_path,
                "single-preview recovered next-Vote body service join",
                next_vote_service_join,
                (
                    "ValidatedBodyReceipt",
                    "V2BodyStore",
                    "prepare_recovered_lifecycle_sign_completion(completion)",
                    "into_parts",
                ),
            )
            next_vote_executor_join = region(
                effects_path,
                effects_source,
                "single-preview recovered next-Vote body executor join",
                "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion_with_body(",
                "/// Reserve exclusive mutation of the exact recovered response-family claim.",
            )
            require_order(
                effects_path,
                "single-preview recovered next-Vote body executor join",
                next_vote_executor_join,
                (
                    "service.consume_for_executor(",
                    "runtime.prepare_recovered_lifecycle_sign_completion(completion)",
                    "preview.project_broadcast_and_sign_body_lookup(",
                    "preview.prepare_proposal_prepare_wal_body_lookup(",
                    "authenticate_recovered_lifecycle_next_vote_body_catalogs(",
                    "Ok((preview, body))",
                ),
            )
            next_vote_catalog_join = region(
                effects_path,
                effects_source,
                "exact recovered next-Vote body catalog join",
                "fn authenticate_recovered_lifecycle_next_vote_body_catalogs(",
                "impl V2EffectExecutor<SerializedV2Runtime>",
            )
            require_tokens(
                effects_path,
                "exact recovered next-Vote body catalog join",
                next_vote_catalog_join,
                (
                    "validated_bodies.get(&key) != Some(&validated)",
                    "durable_bodies.get(&key) != Some(durable)",
                    "recovered_bodies.get(&key)",
                    "HashOf::new(manifest) != durable.manifest_hash()",
                    "lookup.matches_recovered_body(manifest, recovered_durable)",
                    "RecoveredLifecycleNextVoteBodyAuthorityMintPermitV1::new()",
                ),
            )
            next_vote_body_authority = region(
                adapter_path,
                adapter_source,
                "opaque recovered next-Vote body authority",
                "pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyAuthorityV1 {",
                "/// Closed reducer successor shape produced by one exact recovered signature.",
            )
            require_tokens(
                adapter_path,
                "opaque recovered next-Vote body authority",
                next_vote_body_authority,
                (
                    "body_store_identity.same_instance(expected_body_store_identity)",
                    "lookup.matches_adapter_successor(next_sign, expected_proposal_manifest_hash)",
                ),
            )
            reject_tokens(
                adapter_path,
                "opaque recovered next-Vote body authority",
                next_vote_body_authority,
                (
                    "impl Clone for RecoveredLifecycleNextVoteBodyAuthorityV1",
                    "fn into_parts(",
                    "fn validated(",
                    "fn body_store_identity(",
                    "fn lookup(",
                ),
            )
            combined_adapter_projection = region(
                adapter_path,
                adapter_source,
                "affine recovered Broadcast-and-next-Sign adapter projection",
                "pub(in crate::sumeragi) fn project_broadcast_and_sign_authority(",
                "/// Exercise fail-closed next-Sign substitution",
            )
            require_order(
                adapter_path,
                "affine recovered Broadcast-and-next-Sign adapter projection",
                combined_adapter_projection,
                (
                    "self.combined_authority_minted",
                    "body_authority.consume_for_adapter(",
                    "self.persisted_prepare_wal.is_some()",
                    "core::mem::swap(&mut self.adapter.reducer, &mut self.next_reducer)",
                    "core::mem::swap(&mut self.adapter.registry, &mut self.next_registry)",
                    "self.adapter.authenticate_recovered_lifecycle_next_vote(",
                    "core::mem::swap(&mut self.adapter.registry, &mut self.next_registry)",
                    "core::mem::swap(&mut self.adapter.reducer, &mut self.next_reducer)",
                    "self.combined_authority_minted = true",
                    "RecoveredLifecycleSignBroadcastAndSignAuthorityV1 {",
                ),
            )
            proposal_output_authority = region(
                adapter_path,
                adapter_source,
                "opaque recovered Proposal exact-output authority",
                "pub(in crate::sumeragi) struct RecoveredLifecycleProposalExactOutputAuthorityV1 {",
                "/// Adapter-authenticated combined successor of one recovered signature.",
            )
            require_tokens(
                adapter_path,
                "opaque recovered Proposal exact-output authority",
                proposal_output_authority,
                (
                    "body_store_identity: V2BodyStoreInstanceIdentity",
                    "output_guard: Arc<super::output_guard::ConsensusOutputGuard>",
                    "fn consume_for_service(",
                    "fn from_service_retry(",
                    "Self::validated(",
                ),
            )
            reject_tokens(
                adapter_path,
                "opaque recovered Proposal exact-output authority",
                proposal_output_authority,
                (
                    "impl Clone for RecoveredLifecycleProposalExactOutputAuthorityV1",
                    "fn into_parts(",
                    "fn proposal(",
                    "fn payload(",
                    "fn body_store_identity(",
                    "fn output_guard(",
                ),
            )
            proposal_output_projection = region(
                adapter_path,
                adapter_source,
                "affine recovered Proposal exact-output projection",
                "pub(in crate::sumeragi) fn project_proposal_exact_output_authority(",
                "fn broadcast_proposal_manifest_hash(",
            )
            require_order(
                adapter_path,
                "affine recovered Proposal exact-output projection",
                proposal_output_projection,
                (
                    "let shape = self.shape()",
                    "self.proposal_output_authority_minted",
                    "!matches!( shape, RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign | RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal )",
                    "shape == RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
                    "self.prepared_prepare_wal.is_none()",
                    "payload.manifest() == &signed.manifest",
                    "self.next_vote_body_store_identity.as_ref()",
                    "self.next_vote_output_guard.as_ref()",
                    "self.proposal_output_authority_minted = true",
                    "RecoveredLifecycleProposalExactOutputAuthorityV1 {",
                ),
            )
            require_tokens(
                adapter_path,
                "affine recovered Proposal exact-output projection",
                proposal_output_projection,
                (
                    "RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign | RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
                ),
            )
            proposal_prepare_wal_preflight = region(
                adapter_path,
                adapter_source,
                "pre-WAL initial Proposal continuation",
                "pub(in crate::sumeragi) fn prepare_proposal_prepare_wal_body_lookup(",
                "/// Append and fsync the preflighted initial Proposal `PrepareIntent`.",
            )
            require_order(
                adapter_path,
                "pre-WAL initial Proposal continuation",
                proposal_prepare_wal_preflight,
                (
                    "RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
                    "self.pending_prepare.as_ref().cloned()",
                    "expected_wal_sequence.checked_add(1) != Some(entry.id().get())",
                    "encode_wal_entry(&entry, self.adapter.aggregator.as_ref())",
                    "let continuation = next_reducer.step(persisted_event.clone())",
                    "message: reducer::SignableMessage::Vote(vote)",
                    "RecoveredLifecycleNextVoteBodyLookupV1::from_adapter_preview(",
                    "self.next_vote_body_store_identity = Some(body_store_identity)",
                    "self.prepared_prepare_wal = Some(PreparedRecoveredLifecycleProposalPrepareWalV1 {",
                ),
            )
            reject_tokens(
                adapter_path,
                "mutation-free initial Proposal WAL preflight",
                proposal_prepare_wal_preflight,
                (".wal.append(", "self.adapter.reducer =", "self.adapter.registry ="),
            )
            proposal_prepare_wal_append = region(
                adapter_path,
                adapter_source,
                "fail-stop initial Proposal WAL append",
                "pub(in crate::sumeragi) fn append_recovered_lifecycle_proposal_prepare_wal(",
                "/// Project an inert exact-body lookup for the reducer-produced next Vote.",
            )
            require_order(
                adapter_path,
                "fail-stop initial Proposal WAL append",
                proposal_prepare_wal_append,
                (
                    "self.proposal_output_authority_minted",
                    "self.next_vote_body_store_identity.is_none()",
                    "self.next_vote_output_guard.is_none()",
                    "permit.authorizes(",
                    "self.adapter.pending_persistence_id = Some(persistence_id)",
                    "permit.cross_wal_attempt_boundary()",
                    "self.adapter.wal.append(&encoded_wal_payload)",
                    "LiveWalFrameIdentity::from_append_receipt(frame, receipt, persistence_id)",
                    "PendingRuntimeEffectBinding::from_exact_live_wal_append(",
                    "SealedLiveWalPersistedEffectV1::from_exact_live_append(",
                    "self.next_reducer = next_reducer",
                    "self.next_sign = Some(sign_effect)",
                    "self.pending_prepare = None",
                    "self.persisted_prepare_wal = Some(RecoveredLifecycleProposalPrepareWalContinuationV1 {",
                ),
            )
            require_tokens(
                adapter_path,
                "initial Proposal WAL ambiguity closes the adapter",
                proposal_prepare_wal_append,
                ("self.adapter.fail_closed = true", "WalFrameIdentityMismatch"),
            )
            proposal_batch_preflight = region(
                worker_path,
                worker_source,
                "mutation-free atomic Proposal fanout preflight",
                "fn prepare_atomic_fanout_batch(",
                "/// Commit a batch prepared while this exact mutex guard remained held.",
            )
            require_order(
                worker_path,
                "mutation-free atomic Proposal fanout preflight",
                proposal_batch_preflight,
                (
                    "let mut additions = BTreeMap",
                    "aggregate.checked_add(count)",
                    "self.ownership_capacity_available(&additions)?",
                    "self.ownership_state_after_additions(&additions)?",
                    "let project_ids = |first: ExactFanoutFifoId|",
                    "self.source_fifo_owners.clone()",
                    "Some(existing_ids)",
                    "source_fifo_owners.entry(source).or_default().insert(fifo_id)",
                    "PendingExactOutputBatchPlan {",
                ),
            )
            reject_tokens(
                worker_path,
                "mutation-free atomic Proposal fanout preflight",
                proposal_batch_preflight,
                (
                    "self.fanouts.extend(",
                    "self.source_fifo_owners =",
                    "self.reservation_owner_counts =",
                    "self.ownership_units =",
                    "rebase_source_fifo(",
                    "allocate_fanout_fifo_id(",
                    ".enqueue(",
                    "next_fanout_index =",
                ),
            )
            proposal_batch_commit = region(
                worker_path,
                worker_source,
                "assertion-only atomic Proposal fanout commit",
                "fn commit_atomic_fanout_batch(&mut self, plan: PendingExactOutputBatchPlan)",
                "fn is_pending(&self)",
            )
            require_order(
                worker_path,
                "assertion-only atomic Proposal fanout commit",
                proposal_batch_commit,
                (
                    "assert_eq!(self.fanouts.len(), existing_fanout_count",
                    "if let Some(rebased) = rebased_existing_fifo_ids",
                    "fanout.fifo_id = Some(fifo_id)",
                    "self.fanouts.extend(fanouts)",
                    "self.source_fifo_owners = source_fifo_owners",
                    "self.reservation_owner_counts = reservation_owner_counts",
                    "self.ownership_units = ownership_units",
                    "self.shared_ownership_units = shared_ownership_units",
                    "self.next_fanout_fifo_id = next_fanout_fifo_id",
                ),
            )
            reject_tokens(
                worker_path,
                "assertion-only atomic Proposal fanout commit",
                proposal_batch_commit,
                ("?", "drive_pending_exact_output", ".enqueue("),
            )
            proposal_reservation_fields = region(
                worker_path,
                worker_source,
                "fail-stop-first recovered Proposal reservation ownership",
                "pub(in crate::sumeragi) struct RecoveredLifecycleProposalExactOutputReservationV1<'service> {",
                "#[cfg_attr(not(test), allow(dead_code))]\nimpl RecoveredLifecycleProposalExactOutputReservationV1<'_> {",
            )
            require_order(
                worker_path,
                "fail-stop-first recovered Proposal reservation ownership",
                proposal_reservation_fields,
                (
                    "operation: Option<ConsensusFailStopOperation<'service>>",
                    "pending: Option<std::sync::MutexGuard<'service, PendingExactOutput>>",
                    "batch: Option<PendingExactOutputBatchPlan>",
                    "authority: Option<super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1>",
                    "wal_append: RecoveredLifecycleProposalPrepareWalAppendSealV1",
                ),
            )
            proposal_wal_append_seal = region(
                worker_path,
                worker_source,
                "reservation-bound initial Proposal WAL append authority",
                "struct RecoveredLifecycleProposalPrepareWalAppendSealV1 {",
                "#[cfg_attr(not(test), allow(dead_code))]\nimpl RecoveredLifecycleProposalExactOutputReservationV1<'_> {",
            )
            require_order(
                worker_path,
                "reservation-bound initial Proposal WAL append authority",
                proposal_wal_append_seal,
                (
                    "dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1",
                    "body_store_identity: V2BodyStoreInstanceIdentity",
                    "output_guard: Arc<ConsensusOutputGuard>",
                    "attempted: bool",
                    "pub(in crate::sumeragi) struct RecoveredLifecycleProposalPrepareWalAppendPermitV1<'reservation>",
                    "seal: &'reservation mut RecoveredLifecycleProposalPrepareWalAppendSealV1",
                    "!self.seal.attempted",
                    "self.seal.dispatch_key == dispatch_key",
                    ".same_instance(body_store_identity)",
                    "Arc::ptr_eq(&self.seal.output_guard, output_guard)",
                    "pub(in crate::sumeragi) fn cross_wal_attempt_boundary(self)",
                    "self.seal.attempted = true",
                ),
            )
            proposal_reservation_impl = region(
                worker_path,
                worker_source,
                "sealed recovered Proposal reservation methods",
                "impl RecoveredLifecycleProposalExactOutputReservationV1<'_> {",
                "pub(in crate::sumeragi) struct RecoveredDecisionFetchExactOutputReservationV1<'service> {",
            )
            require_order(
                worker_path,
                "armed Proposal reservation lends WAL authority without parts",
                proposal_reservation_impl,
                (
                    "pub(in crate::sumeragi) fn prepare_wal_append_permit(",
                    "self.operation.is_some()\n            && self.pending.is_some()\n            && self.batch.is_some()\n            && self.authority.is_some()\n            && !self.wal_append.attempted",
                    "seal: &mut self.wal_append",
                ),
            )
            proposal_reservation_abort = region(
                worker_path,
                proposal_reservation_impl,
                "retry-safe recovered Proposal reservation abort",
                "pub(in crate::sumeragi) fn abort_before_publication(",
                "/// Install both preflighted fanouts in one assertion-only publication tail.",
            )
            require_order(
                worker_path,
                "retry-safe recovered Proposal reservation abort",
                proposal_reservation_abort,
                (
                    "assert!(\n            !self.wal_append.attempted",
                    "drop(self.pending.take())",
                    "drop(self.batch.take())",
                    ".complete()",
                    "self.authority.take()",
                ),
            )
            proposal_reservation_commit = proposal_reservation_impl.split(
                "/// Install both preflighted fanouts in one assertion-only publication tail.",
                1,
            )[-1]
            require_order(
                worker_path,
                "assertion-only recovered Proposal reservation commit",
                proposal_reservation_commit,
                (
                    "let mut pending = self.pending.take()",
                    "let operation = self.operation.take()",
                    "let batch = self.batch.take()",
                    "let authority = self.authority.take()",
                    "pending.commit_atomic_fanout_batch(batch)",
                    "drop(pending)",
                    "drop(authority)",
                    "operation.complete()",
                ),
            )
            reject_tokens(
                worker_path,
                "sealed recovered Proposal reservation methods",
                proposal_reservation_abort + proposal_reservation_commit,
                ("drive_pending_exact_output", ".enqueue("),
            )
            proposal_output_capture = region(
                worker_path,
                worker_source,
                "retry-safe recovered Proposal exact-output capture",
                "pub(in crate::sumeragi) fn capture_recovered_lifecycle_proposal_exact_output(",
                "/// Consume one carrier-derived recovered Fetch through this exact service key.",
            )
            require_order(
                worker_path,
                "retry-safe recovered Proposal exact-output capture",
                proposal_output_capture,
                (
                    "self.proposal_work_retired",
                    "authority.consume_for_service(RecoveredLifecycleProposalExactOutputPermitV1::new())",
                    "tag != self.active_tag",
                    "self.local_validator != Some(proposal.proposer)",
                    "proposal.manifest != *payload.manifest()",
                    "identity.same_instance(&body_store_identity)",
                    "Arc::ptr_eq(&self.output_guard, &authority_output_guard)",
                    "message.validate_version()",
                    "proposal.validate(&self.context)",
                    "let wal_append = RecoveredLifecycleProposalPrepareWalAppendSealV1 {",
                    "body_store_identity: body_store_identity.clone()",
                    "output_guard: Arc::clone(&authority_output_guard)",
                    "RecoveredLifecycleProposalExactOutputAuthorityV1::from_service_retry(",
                    "payload.into_parts()",
                    "manifest.validate(&self.context)",
                    "chunk.signature_preimage(&self.context, &manifest)",
                    "Signature::try_new(self.key_pair.private_key(), &preimage)",
                    "let peers = self.remote_voters()",
                    "let control = PendingExactFanout::claimed(",
                    "ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope())",
                    "let chunks = PendingExactFanout::claimed(",
                    "ExactOutputRolloverClaim::PayloadChunks",
                    "control.into_iter().chain(chunks)",
                    "begin_fail_stop_operation()",
                    "let pending = self.lock_pending_exact_output()?",
                    "pending.prepare_atomic_fanout_batch(fanouts)",
                    "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(retry_authority,)",
                    "RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(",
                    "authority: Some(retry_authority)",
                    "wal_append",
                ),
            )
            require_token_count(
                worker_path,
                "fail-stop recovered Proposal capture errors",
                proposal_output_capture,
                "drop(operation)",
                2,
            )
            reject_tokens(
                worker_path,
                "all-voter recovered Proposal retransmission policy",
                proposal_output_capture,
                ("fast_path_proposals", "remote_voters_for_indices"),
            )
            broadcast_consensus = region(
                worker_path,
                worker_source,
                "production consensus broadcast",
                "fn broadcast_consensus(",
                "fn sign_body_request(",
            )
            proposal_live_atomic = region(
                worker_path,
                broadcast_consensus,
                "live Proposal control-plus-chunk atomic transfer",
                "if let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload {",
                "let control = vec![Self::preencode_v2_network_message(message)?]",
            )
            require_order(
                worker_path,
                "live Proposal control-plus-chunk atomic transfer",
                proposal_live_atomic,
                (
                    "self.outbound_chunks.get(&manifest_hash)",
                    "let first_fast_path_send = !self.fast_path_proposals.contains(&proposal.round)",
                    "PendingExactFanout::claimed(",
                    "ExactOutputRolloverClaim::PayloadChunks",
                    "self.enqueue_atomic_fanout_batch_while_guarded(",
                    "ownership == ExactFanoutOwnership::Owned && first_fast_path_send",
                    "self.fast_path_proposals.insert(proposal.round)",
                ),
            )
            reject_tokens(
                worker_path,
                "live Proposal control-plus-chunk atomic transfer",
                proposal_live_atomic,
                (
                    "enqueue_exact_fanout_while_guarded(",
                    "self.fast_path_proposals.insert(proposal.round);\n            let payload_targets",
                ),
            )
            require_tokens(
                worker_path,
                "atomic Proposal output behavior regressions",
                worker_source,
                (
                    "fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound()",
                    "fn atomic_fanout_batch_preflights_aggregate_capacity_and_rebases_only_on_commit()",
                    "fn armed_recovered_proposal_output_reservation_fails_stop_on_drop()",
                    "fn proposal_broadcast_reports_source_retained_until_corridor_acceptance()",
                ),
            )
            proposal_output_behavior = region(
                worker_path,
                worker_source,
                "recovered Proposal atomic output behavior",
                "fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound()",
                "fn prepare_and_commit_votes_reach_every_remote_voter_across_views()",
            )
            require_tokens(
                worker_path,
                "recovered Proposal atomic output behavior",
                proposal_output_behavior,
                (
                    "after, before",
                    "vec![Some(expected_batch_first_fifo), expected_batch_first_fifo.checked_add(1),]",
                    "fanout.peers.iter().cloned().collect::<BTreeSet<_>>()",
                    "wire::ConsensusMessageV2Payload::PayloadChunk(chunk)",
                    "chunk.validate(&service.context, manifest)",
                    "Signature::try_from_bytes(&chunk.signature)",
                    "signature.verify(signer.public_key()",
                    "capture_recovered_lifecycle_proposal_exact_output(retirement_authority).is_err()",
                ),
            )
            require_order(
                worker_path,
                "post-Decision live Proposal output fence",
                broadcast_consensus,
                (
                    "self.proposal_work_retired",
                    "wire::ConsensusMessageV2Payload::Proposal(_)",
                    "begin_fail_stop_operation()",
                    "if let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload",
                ),
            )
            next_vote_candidate_projection = region(
                replay_authority_path,
                replay_authority_source,
                "full executable recovered next-WAL-Vote candidate",
                "pub(in crate::sumeragi) fn into_candidate_projection(",
                "/// Rejoin the retained body marker to one exact recovered phase-vote repair.",
            )
            require_order(
                replay_authority_path,
                "full executable recovered next-WAL-Vote candidate",
                next_vote_candidate_projection,
                (
                    "self.wal_identity.is_exact()",
                    "self.matches_verified_height(verified)",
                    "PendingRuntimeEffectBinding::from_exact_recovered_next_wal_vote(",
                    "self.replay_evidence.project_recovered_vote_candidate(",
                    "RecoveredLifecycleNextWalVoteCandidateProjectionV1 {",
                    "projection.is_exact(verified)",
                ),
            )
            require_tokens(
                runtime_path,
                "runtime-private recovered next-WAL-Vote candidate mint",
                runtime_source,
                (
                    "fn project_recovered_lifecycle_next_wal_vote_candidate(",
                    "RecoveredLifecycleNextWalVoteCandidateProjectionPermitV1::new()",
                    "RecoveredWalCandidateProjectionPermit::new()",
                ),
            )
            require_tokens(
                wal_recovery_path,
                "WAL-bound recovered Broadcast-and-next-Sign projection",
                wal_recovery_source,
                (
                    "fn project_authenticated_signed_broadcast_and_sign(",
                    "next_sign.matches_verified_height(verified)",
                    "next_sign.matches_phase_vote_repair(self)",
                    "project_recovered_lifecycle_next_wal_vote_candidate(verified, next_sign)",
                    "combined.children_are_exact(verified)",
                ),
            )
            combined_cold_projection = region(
                wal_recovery_path,
                wal_recovery_source,
                "affine recovered Broadcast-and-next-Sign cold adapter projection",
                "impl RecoveredLifecycleSignedBroadcastAndSignProjectionV1 {",
                "fn project_recovered_signed_broadcast(",
            )
            require_order(
                wal_recovery_path,
                "affine recovered Broadcast-and-next-Sign cold adapter projection",
                combined_cold_projection,
                (
                    "self.cold_adapter_authority_minted",
                    "self.children_are_exact(verified)",
                    "self.next_sign.project_cold_adapter_next_sign(",
                    "RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(",
                    "self.cold_adapter_authority_minted = true",
                    "candidates.get(&self.broadcast.candidate.key) == Some(&self.broadcast.candidate)",
                    "self.next_sign.owns_spliced_candidate(candidates)",
                ),
            )
            reject_tokens(
                wal_recovery_path,
                "affine recovered Broadcast-and-next-Sign cold adapter projection",
                combined_cold_projection,
                (
                    "fn into_parts(",
                    "pub fn broadcast(",
                    "pub fn next_sign(",
                    "candidates.len() == 2",
                ),
            )
            next_vote_cold_projection = region(
                replay_authority_path,
                replay_authority_source,
                "sealed recovered next-WAL-Vote cold adapter projection",
                "pub(super) fn project_cold_adapter_next_sign(",
                "/// Return the exact installed effect digest",
            )
            require_order(
                replay_authority_path,
                "sealed recovered next-WAL-Vote cold adapter projection",
                next_vote_cold_projection,
                (
                    "RecoveredLifecycleSignBroadcastProjectionPermitV1",
                    "self.is_exact(verified)",
                    "self.seal.effect.clone()",
                ),
            )
            combined_cold_authority = region(
                adapter_path,
                adapter_source,
                "opaque recovered Broadcast-and-next-Sign cold adapter authority",
                "pub(in crate::sumeragi) struct RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1 {",
                "impl RecoveredLifecycleSignColdAdapterAuthorityV1",
            )
            require_tokens(
                adapter_path,
                "opaque recovered Broadcast-and-next-Sign cold adapter authority",
                combined_cold_authority,
                (
                    "broadcast: AdapterEffect",
                    "next_sign: AdapterEffect",
                    "RecoveredLifecycleSignBroadcastProjectionPermitV1",
                    "ConsensusMessageV2Payload::Proposal(proposal)",
                    "ConsensusMessageV2Payload::Vote(vote)",
                    "GlobalPhase::Prepare => tag.view() == next_vote.round.view",
                    "GlobalPhase::Commit => tag.view() >= next_vote.round.view",
                    "relation_is_exact.then_some(Self",
                ),
            )
            reject_tokens(
                adapter_path,
                "opaque recovered Broadcast-and-next-Sign cold adapter authority",
                combined_cold_authority,
                (
                    "fn into_parts(",
                    "fn broadcast(",
                    "fn next_sign(",
                    "impl Clone for RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1",
                ),
            )
            combined_cold_adapter = region(
                adapter_path,
                adapter_source,
                "recovered Broadcast-and-next-Sign cold adapter replay",
                "pub(in crate::sumeragi) fn advance_recovered_lifecycle_signed_broadcast_and_sign(",
                "/// Seal every adapter-owned input required by the adjacent gate open.",
            )
            require_order(
                adapter_path,
                "recovered Broadcast-and-next-Sign cold adapter replay",
                combined_cold_adapter,
                (
                    "verified.verify_consensus_message(message)",
                    "adapter.reducer.awaiting_signature()",
                    "let outcome = next_reducer.step(event.clone())",
                    "replayed_broadcast != broadcast",
                    "replayed_next_sign != next_sign",
                    "adapter.reducer = next_reducer",
                    "adapter.registry = next_registry",
                ),
            )
            reject_tokens(
                adapter_path,
                "recovered Broadcast-and-next-Sign cold adapter replay",
                combined_cold_adapter,
                ("publish_status", ".append(", "broadcast_consensus", "enqueue("),
            )
            combined_ledger_classifier = region(
                ledger_operations_path,
                ledger_operations_source,
                "frame-bound recovered Broadcast-and-next-Sign ledger classifier",
                "pub(in crate::sumeragi) fn recovered_lifecycle_signed_broadcast_and_sign_pairs(",
                "/// Stage the exact all-row tombstone successor for finalized-height retirement.",
            )
            require_tokens(
                ledger_operations_path,
                "frame-bound recovered Broadcast-and-next-Sign ledger classifier",
                combined_ledger_classifier,
                (
                    "self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?",
                    "let ledger_frame_identity = self.frame_identity()",
                    "RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1::new(&self.records)",
                    "index.unique_parent_index(broadcast_ordinal)",
                    "index.owner_record_count(next_sign_owner) != 1",
                    "index.has_incoming_edge(next_sign_ordinal)",
                    "let next_sign_ordinal = broadcast_ordinal.checked_add(1)?",
                    "signed_broadcast_continuation_is_exact(",
                    "recovered_broadcast_and_next_sign_keys_are_exact(",
                    "next_sign_owner.first_admission_ordinal() != next_sign_ordinal",
                    "parent_record_count == 2",
                    "parent_record_count == 3",
                    "DurableContinuationEdge::ValidateToSignPrepare",
                    "ledger_frame_identity",
                ),
            )
            combined_ledger_enumerator = region(
                ledger_operations_path,
                ledger_operations_source,
                "linear recovered Broadcast-and-next-Sign ledger enumeration",
                "pub(in crate::sumeragi) fn recovered_lifecycle_signed_broadcast_and_sign_pairs(",
                "fn project_recovered_lifecycle_signed_broadcast_and_sign_at(",
            )
            require_order(
                ledger_operations_path,
                "linear recovered Broadcast-and-next-Sign ledger enumeration",
                combined_ledger_enumerator,
                (
                    "self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?",
                    "let ledger_frame_identity = self.frame_identity()",
                    "RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1::new(&self.records)",
                    "self.records.iter()",
                    "project_validated_recovered_lifecycle_signed_broadcast_and_sign_at(",
                    "&index",
                ),
            )
            require_token_count(
                ledger_operations_path,
                "linear recovered Broadcast-and-next-Sign ledger enumeration",
                combined_ledger_enumerator,
                "self.frame_identity()",
                1,
            )
            reject_tokens(
                ledger_operations_path,
                "frame-bound recovered Broadcast-and-next-Sign ledger classifier",
                combined_ledger_classifier,
                ("high_water == next_sign_ordinal", "persist_exact_successor"),
            )
            combined_ledger_reauth = region(
                ledger_path,
                ledger_source,
                "single-hash recovered Broadcast-and-next-Sign ledger reauthentication",
                "pub(in crate::sumeragi) fn exactly_matches_ledger(&self, ledger: &LifecycleLedgerV1) -> bool {",
                "/// Complete version-one durable lifecycle ledger.",
            )
            require_tokens(
                ledger_path,
                "single-hash recovered Broadcast-and-next-Sign ledger reauthentication",
                combined_ledger_reauth,
                (
                    "project_recovered_lifecycle_signed_broadcast_and_sign_at(self.broadcast_ordinal)",
                    "== Some(self)",
                ),
            )
            reject_tokens(
                ledger_path,
                "single-hash recovered Broadcast-and-next-Sign ledger reauthentication",
                combined_ledger_reauth,
                ("ledger.frame_identity()",),
            )
            combined_registry_prepare = region(
                registry_path,
                registry_source,
                "opaque recovered Broadcast-and-next-Sign registry preparation",
                "pub(super) fn prepare_recovered_lifecycle_sign_broadcast_and_sign_successor<",
                "impl<'registry, 'adapter> PreparedRecoveredLifecycleSignBroadcastSuccessor<'registry, 'adapter>",
            )
            require_order(
                registry_path,
                "opaque recovered Broadcast-and-next-Sign registry preparation",
                combined_registry_prepare,
                (
                    "adapter.dispatch_key() != key",
                    "sign.matches_claimed_record(",
                    "adapter.project_broadcast_and_sign_authority(body)",
                    ".project_authenticated_signed_broadcast_and_sign(verified, projection_authority)",
                    "PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor {",
                ),
            )
            reject_tokens(
                registry_path,
                "unpublished recovered Broadcast-and-next-Sign registry preparation",
                combined_registry_prepare,
                (
                    "ValidatedBodyReceipt",
                    "into_parts",
                    "entries.insert",
                    "entries.remove",
                    "persist_exact_successor",
                ),
            )
            ordinary_body_transition = _require_rust_item(
                body_pipeline_path,
                body_pipeline_source,
                "stage_body_stage_transition",
                errors,
            )
            if ordinary_body_transition is not None:
                require_tokens(
                    body_pipeline_path,
                    "ordinary adjacent body successor reserves exactly one ordinal",
                    ordinary_body_transition.source,
                    (
                        "BodyStagePayloadRelationV1::OrdinaryBodyFrame, 1",
                    ),
                )
            recovered_decision_transition = _require_rust_item(
                body_pipeline_path,
                body_pipeline_source,
                "stage_recovered_decision_fetch_store_transition",
                errors,
            )
            if recovered_decision_transition is not None:
                require_tokens(
                    body_pipeline_path,
                    "recovered Decision Fetch-to-Store reserves exactly one ordinal",
                    recovered_decision_transition.source,
                    (
                        "BodyStagePayloadRelationV1::RecoveredDecisionFetch, 1",
                    ),
                )
            recovered_single_broadcast_transition = _require_rust_item(
                body_pipeline_path,
                body_pipeline_source,
                "prepare_recovered_lifecycle_sign_broadcast_transition",
                errors,
            )
            if recovered_single_broadcast_transition is not None:
                require_tokens(
                    body_pipeline_path,
                    "single recovered Broadcast reserves exactly one ordinal",
                    recovered_single_broadcast_transition.source,
                    (
                        "stage_recovered_lifecycle_sign_broadcast_transition(self, lease, candidate, 1)",
                    ),
                )
            body_transition_reservation = _require_rust_item(
                body_pipeline_path,
                body_pipeline_source,
                "stage_body_stage_transition_with_payload_relation",
                errors,
            )
            if body_transition_reservation is not None:
                require_order(
                    body_pipeline_path,
                    "adjacent body successor retains its actor-global ordinal reservation",
                    body_transition_reservation.source,
                    (
                        "ordinal_count: usize",
                        "reserve_body_transition_ordinals(coordinator, ordinal_count)",
                        "ordinal_reservation.first()",
                        "staged.reduce_admit_with_ordinal_allocator",
                        "count != 1 || expected_child_ordinal <= high_water",
                        "Ok(StagedBodyStageTransition { staged, ordinal_reservation",
                    ),
                )
            combined_transition = region(
                body_pipeline_path,
                body_pipeline_source,
                "inert recovered Broadcast-and-next-Sign coordinator staging",
                "fn stage_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                "#[allow(clippy::too_many_arguments, clippy::too_many_lines)]\nfn stage_body_stage_transition_with_payload_relation(",
            )
            require_order(
                body_pipeline_path,
                "inert recovered Broadcast-and-next-Sign coordinator staging",
                combined_transition,
                (
                    "stage_recovered_lifecycle_sign_broadcast_transition(coordinator, lease, broadcast, 2)",
                    "ordinal_reservation.last()",
                    "broadcast_ordinal.checked_add(1)",
                    "staged.reduce_admit_with_ordinal_allocator( AdmissionRequest::Candidate(next_sign)",
                    "count != 1 || expected_next_sign_ordinal <= high_water",
                    "Ok((expected_next_sign_ordinal, expected_next_sign_ordinal))",
                    "next_sign_owner == broadcast_owner",
                    "staged.high_water != next_sign_ordinal",
                    "capacity_generation_before[&CapacityClass::Effect].saturating_add(1)",
                    "capacity_used_before[&CapacityClass::Consensus].saturating_add(1)",
                    "Ok(StagedRecoveredLifecycleSignBroadcastAndSignTransition {",
                ),
            )
            reject_tokens(
                body_pipeline_path,
                "inert recovered Broadcast-and-next-Sign coordinator staging",
                combined_transition,
                (
                    "persist_exact_successor",
                    "commit_after_publication",
                    "registry.entries",
                ),
            )
            combined_transition_publication = region(
                body_pipeline_path,
                body_pipeline_source,
                "durable recovered Broadcast-and-next-Sign publication",
                "impl PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'_, '_, '_> {",
                "fn map_sealed_successor_projection_error(",
            )
            require_order(
                body_pipeline_path,
                "durable recovered Broadcast-and-next-Sign publication",
                combined_transition_publication,
                (
                    "persist_exact_staged_successor_with_ordinal_reservation( &self.staged, &self.ordinal_reservation, )",
                    "successor.commit_after_publication()",
                    "*coordinator = staged",
                    "if publication_is_vote",
                    "ready_index.contains(&next_sign_ordinal)",
                    "adapter.commit_after_durable_vote_broadcast_and_sign()",
                ),
            )
            require_tokens(
                body_pipeline_path,
                "Proposal publication parks only its durable Broadcast debt",
                combined_transition_publication,
                (
                    "ready_index.remove(&broadcast_ordinal)",
                    "LifecycleState::Waiting(broadcast_wait)",
                    "adapter.commit_after_durable_broadcast_and_sign()",
                ),
            )
            combined_transition_tail = combined_transition_publication.split(
                "successor.commit_after_publication()", 1
            )[-1]
            reject_tokens(
                body_pipeline_path,
                "infallible recovered Proposal two-child publication tail",
                combined_transition_tail,
                ("return", "is_err", "Result"),
            )
            combined_adapter_commit = region(
                adapter_path,
                adapter_source,
                "durable recovered Proposal adapter two-child commit",
                "pub(in crate::sumeragi) fn commit_after_durable_broadcast_and_sign(self)",
                "/// Borrow-bound adapter successor for one registry-owned lifecycle Apply",
            )
            require_order(
                adapter_path,
                "durable recovered Proposal adapter two-child commit",
                combined_adapter_commit,
                (
                    "RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign",
                    "next_sign: Some(_)",
                    "combined_authority_minted: true",
                    "proposal_output_authority_minted: true",
                    "persisted_prepare_wal",
                    "outbound_payload: Some(_)",
                    "adapter.pending_persistence_id = None",
                    "adapter.reducer = next_reducer",
                    "adapter.registry = next_registry",
                    "adapter.record_reducer_outcome(&persisted_event",
                ),
            )
            combined_vote_adapter_commit = region(
                adapter_path,
                adapter_source,
                "durable recovered Vote adapter two-child commit",
                "pub(in crate::sumeragi) fn commit_after_durable_vote_broadcast_and_sign(self)",
                "/// Borrow-bound adapter successor for one registry-owned lifecycle Apply",
            )
            require_order(
                adapter_path,
                "durable recovered Vote adapter two-child commit",
                combined_vote_adapter_commit,
                (
                    "self.is_vote_broadcast_and_sign()",
                    "next_sign: Some(_)",
                    "combined_authority_minted: true",
                    "proposal_output_authority_minted: false",
                    "outbound_payload: None",
                    "adapter.reducer = next_reducer",
                    "adapter.registry = next_registry",
                ),
            )
            require_tokens(
                registry_validate_path,
                "follow-on recovered WAL Vote remains an executable Sign carrier",
                registry_validate_source,
                (
                    "ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign)",
                    "PreparedRecoveredLifecycleSignCarrier::NextWalVote(sign)",
                ),
            )
            single_broadcast_prepare = region(
                registry_path,
                registry_source,
                "shared-ordinal recovered Sign-to-Broadcast preparation",
                "pub(super) fn prepare_recovered_lifecycle_sign_broadcast_successor",
                "/// Seal the exact Broadcast-and-next-WAL-Sign pair",
            )
            reject_tokens(
                registry_path,
                "shared-ordinal recovered Sign-to-Broadcast preparation",
                single_broadcast_prepare,
                ("coordinator.high_water", ".checked_add(1)", "broadcast_address"),
            )
            single_broadcast_bind = region(
                registry_path,
                registry_source,
                "staged recovered Broadcast registry binding",
                "impl<'registry, 'adapter> PreparedRecoveredLifecycleSignBroadcastSuccessor<'registry, 'adapter>",
                "impl<'adapter> BoundRecoveredLifecycleSignBroadcastSuccessor<'_, 'adapter>",
            )
            require_order(
                registry_path,
                "staged recovered Broadcast registry binding",
                single_broadcast_bind,
                (
                    "exact_staged_recovered_lifecycle_broadcast_address(",
                    "pub(super) fn exact_staged_recovered_lifecycle_broadcast_address(",
                    "coordinator.records.get(&child_ordinal)",
                    "ConcreteWorkAddress::new(record.owner, child_ordinal, child_slot)",
                    "broadcast.matches_current_ready_record(",
                    ".validates_at(verified, broadcast_address, child_digest)",
                    "registry.entries.contains_key(&broadcast_address)",
                ),
            )
            single_broadcast_transition = region(
                body_pipeline_path,
                body_pipeline_source,
                "staged recovered Sign-to-Broadcast transition",
                "pub(super) fn prepare_recovered_lifecycle_sign_broadcast_transition",
                "/// Stage the exact two-child result",
            )
            require_order(
                body_pipeline_path,
                "staged recovered Sign-to-Broadcast transition",
                single_broadcast_transition,
                (
                    "stage_recovered_lifecycle_sign_broadcast_transition(",
                    ".bind_staged_child(",
                ),
            )
            require_tokens(
                body_pipeline_path,
                "staged recovered Sign-to-Broadcast transition",
                single_broadcast_transition,
                ("transition.child_ordinal",),
            )
            fresh_serve_preflight = region(
                registry_path,
                registry_source,
                "shared-ordinal fresh Certified-Serve preflight",
                "pub(super) fn preflights_fresh_registry(",
                "fn exactly_matches_fresh_staged_append(",
            )
            reject_tokens(
                registry_path,
                "shared-ordinal fresh Certified-Serve preflight",
                fresh_serve_preflight,
                ("current.high_water.checked_add(2)",),
            )
            fresh_serve_pair = region(
                registry_path,
                registry_source,
                "adjacent fresh Certified-Serve pair after a shared gap",
                "fn exactly_matches_fresh_staged_append(",
                "\n}\nfn recovered_serve_pairs_preflight(",
            )
            require_tokens(
                registry_path,
                "adjacent fresh Certified-Serve pair after a shared gap",
                fresh_serve_pair,
                (
                    "serve <= current.high_water",
                    "serve.checked_add(1) != Some(producer)",
                    "producer != staged.high_water",
                ),
            )
            recovered_sign_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Sign-to-Broadcast settlement",
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_sign_broadcast(",
                "/// Settle a recovered Prepare Vote into Broadcast plus Commit Sign.",
            )
            require_order(
                launch_path,
                "restart-closed recovered Sign-to-Broadcast settlement",
                recovered_sign_settlement,
                (
                    "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
                    "prepare_recovered_lifecycle_sign_completion(authority)",
                    "prepare_recovered_lifecycle_sign_broadcast_successor(",
                    "prepare_recovered_lifecycle_sign_broadcast_transition(",
                    "output_guard.begin_fail_stop_operation()",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "completion.acknowledge_after_publication()",
                    "operation.complete()",
                ),
            )
            require_tokens(
                launch_path,
                "restart-closed recovered Sign-to-Broadcast settlement",
                recovered_sign_settlement,
                (
                    "ProductionRecoveredLifecycleSignBroadcastSettlementV1::RestartRequired",
                    "ProductionRecoveredLifecycleSignBroadcastSettlementV1::Applied",
                ),
            )
            reject_tokens(
                launch_path,
                "durable recovered Sign-to-Broadcast settlement leaves output to its child",
                recovered_sign_settlement,
                (
                    "capture_recovered_lifecycle_signed_broadcast_refanout",
                    "output.commit_after_publication()",
                    "TurnOutcome::Terminal",
                ),
            )
            recovered_sign_tail = recovered_sign_settlement.split(
                "transition.commit_after_publication();", 1
            )[-1]
            reject_tokens(
                launch_path,
                "infallible recovered Sign-to-Broadcast post-fsync tail",
                recovered_sign_tail,
                ("return", "Result", "is_err"),
            )
            recovered_vote_two_child_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Vote Broadcast-and-next-Sign settlement",
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_vote_broadcast_and_sign(",
                "/// Fsync an initial Proposal `PrepareIntent`, then publish both successors.",
            )
            require_order(
                launch_path,
                "restart-closed recovered Vote Broadcast-and-next-Sign settlement",
                recovered_vote_two_child_settlement,
                (
                    "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
                    "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
                    "preview.is_vote_broadcast_and_sign_shape()",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                    "output_guard.begin_fail_stop_operation()",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "completion.acknowledge_after_publication()",
                    "operation.complete()",
                    "ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::Applied",
                ),
            )
            reject_tokens(
                launch_path,
                "Vote settlement leaves durable output to typed refanout",
                recovered_vote_two_child_settlement,
                (
                    "project_proposal_exact_output_authority",
                    "capture_recovered_lifecycle_proposal_exact_output",
                    "output.commit_after_publication()",
                    "TurnOutcome::Terminal",
                ),
            )
            recovered_vote_two_child_tail = recovered_vote_two_child_settlement.split(
                "transition.commit_after_publication();", 1
            )[-1]
            reject_tokens(
                launch_path,
                "infallible recovered Vote two-child post-fsync tail",
                recovered_vote_two_child_tail,
                ("return", "Result", "is_err", "?"),
            )
            recovered_proposal_prepare_wal_settlement = region(
                launch_path,
                launch_source,
                "restart-closed initial Proposal PrepareIntent settlement",
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_prepare_wal(",
                "/// Settle a recovered Proposal into one Broadcast and one WAL-backed Sign.",
            )
            require_order(
                launch_path,
                "restart-closed initial Proposal PrepareIntent settlement",
                recovered_proposal_prepare_wal_settlement,
                (
                    "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
                    "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
                    "RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
                    "preview.project_proposal_exact_output_authority()",
                    "capture_recovered_lifecycle_proposal_exact_output(output_authority)",
                    "output.prepare_wal_append_permit()",
                    "preview.append_recovered_lifecycle_proposal_prepare_wal(wal_permit)",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "completion.acknowledge_after_publication()",
                    "output.commit_after_publication()",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied",
                ),
            )
            require_tokens(
                launch_path,
                "initial Proposal capacity remains pre-WAL retryable",
                recovered_proposal_prepare_wal_settlement,
                (
                    "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)",
                    "*pending_lifecycle_completion = Some(PendingLifecycleCompletionV1::RecoveredSign(completion))",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::CapacityUnavailable",
                ),
            )
            reject_tokens(
                launch_path,
                "post-WAL initial Proposal never releases fail-stop output",
                recovered_proposal_prepare_wal_settlement.split(
                    "append_recovered_lifecycle_proposal_prepare_wal(wal_permit)", 1
                )[-1],
                ("output.abort_before_publication()",),
            )
            recovered_proposal_prepare_wal_tail = recovered_proposal_prepare_wal_settlement.split(
                "transition.commit_after_publication();", 1
            )[-1]
            reject_tokens(
                launch_path,
                "infallible initial Proposal post-Ledger tail",
                recovered_proposal_prepare_wal_tail,
                ("return", "Result", "is_err", "?"),
            )
            recovered_proposal_two_child_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Proposal Broadcast-and-next-Sign settlement",
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_broadcast_and_sign(",
                "pub(in crate::sumeragi) fn drive_lifecycle_decision_apply_deferred(",
            )
            require_order(
                launch_path,
                "restart-closed recovered Proposal Broadcast-and-next-Sign settlement",
                recovered_proposal_two_child_settlement,
                (
                    "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
                    "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
                    "preview.project_proposal_exact_output_authority()",
                    "capture_recovered_lifecycle_proposal_exact_output(output_authority)",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "completion.acknowledge_after_publication()",
                    "output.commit_after_publication()",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied",
                ),
            )
            require_token_count(
                launch_path,
                "typed recovered Proposal pre-fsync output release",
                recovered_proposal_two_child_settlement,
                "output.abort_before_publication()",
                2,
            )
            require_tokens(
                launch_path,
                "restart-closed recovered Proposal Broadcast-and-next-Sign settlement",
                recovered_proposal_two_child_settlement,
                (
                    "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::CapacityUnavailable",
                    "*pending_lifecycle_completion = Some(PendingLifecycleCompletionV1::RecoveredSign(completion))",
                    "drop(output)",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::RestartRequired",
                ),
            )
            recovered_proposal_two_child_tail = recovered_proposal_two_child_settlement.split(
                "transition.commit_after_publication();", 1
            )[-1]
            reject_tokens(
                launch_path,
                "infallible recovered Proposal two-child post-fsync tail",
                recovered_proposal_two_child_tail,
                ("return", "Result", "is_err", "?"),
            )
            recovered_broadcast_refanout = region(
                scheduler_path,
                scheduler_source,
                "restart-safe recovered signed-Broadcast refanout",
                "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
                "fn persist_recovered_decision_fetch_response_after_runner(",
            )
            require_order(
                scheduler_path,
                "restart-safe recovered signed-Broadcast refanout",
                recovered_broadcast_refanout,
                (
                    "services.matches_lifecycle_body_store(body_store_identity)",
                    "if exact_ready != self.coordinator.ready_index",
                    "record.work_class != LifecycleWorkClass::Broadcast",
                    "recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal",
                    "attest_ready_recovered_lifecycle_signed_broadcast",
                    "for ready_ordinal in &exact_ready",
                    "attest_ready_recovered_lifecycle_sign(",
                    "self.coordinator.plan_turn(inputs)",
                    "project_claimed_recovered_lifecycle_signed_broadcast_output",
                    "capture_recovered_lifecycle_signed_broadcast_refanout(authority)",
                    "let wait_source = super::WaitSource::Recovery(wait_digest)",
                    "settle_turn(lease, super::TurnOutcome::Blocked(wait))",
                    "output.commit_after_publication()",
                ),
            )
            require_tokens(
                scheduler_path,
                "restart-safe recovered signed-Broadcast refanout",
                recovered_broadcast_refanout,
                (
                    "rollback_unpublished_turn(&lease)",
                    "close_admission_for_restart()",
                    "ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::CapacityUnavailable",
                    "ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::RestartRequired",
                    "ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned",
                    "attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote(",
                ),
            )
            reject_tokens(
                scheduler_path,
                "volatile recovered signed-Broadcast refanout wait",
                recovered_broadcast_refanout,
                (
                    "persist_exact_successor",
                    "TurnOutcome::Terminal",
                    "exact_ready.len() == 2",
                    "exact_ready.len() != 2",
                ),
            )
            require_tokens(
                registry_validate_path,
                "retained recovered Broadcast-and-next-Vote pair seal",
                registry_validate_source,
                (
                    "fn recovered_lifecycle_signed_broadcast_declares_next_vote(",
                    "fn recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(",
                    "let (next, next_digest) = broadcast.paired_next_sign?",
                    "next_record.physical_slots.get(&next.slot) == Some(&next_digest)",
                    "self.recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal( coordinator, broadcast_ordinal, ) != Some(next_sign_ordinal)",
                    "DurableRecoveredLifecycleNextWalVoteSign(next_sign)",
                ),
            )
            require_tokens(
                worker_path,
                "durable recovered signed-Broadcast service capture",
                worker_source,
                (
                    "fn capture_recovered_lifecycle_signed_broadcast_refanout(",
                    "authority.consume_for_service(RecoveredLifecycleSignBroadcastOutputPermitV1::new())",
                    "PendingExactFanout::claimed(",
                    "pending.can_enqueue(fanout)",
                    "fn capture_recovered_lifecycle_cold_proposal_message(",
                    "output.consume_for_service(RecoveredLifecycleProposalExactOutputPermitV1::new())",
                    "self.proposal_work_retired",
                    "pending.prepare_atomic_fanout_batch(fanouts)",
                    "cold_durable_proposal_refanout_atomically_owns_control_and_chunks",
                ),
            )
            require_tokens(
                ledger_path,
                "cold recovered signed-Broadcast ledger join",
                ledger_source,
                (
                    "fn authenticate_recovered_control_signed_broadcast(",
                    "fn authenticate_recovered_phase_signed_broadcast_repair(",
                    "project_recovered_signed_broadcast_child(self.context())",
                    "recover_durable_signed_broadcast(verified, child)",
                    "broadcast.exactly_matches_record(",
                ),
            )
            require_tokens(
                wal_recovery_path,
                "cold recovered signed-Broadcast WAL and roster join",
                wal_recovery_source,
                (
                    "fn recover_durable_signed_broadcast(",
                    "verified.verify_consensus_message(message)",
                    "fn project_cold_adapter_authority(",
                    "RecoveredLifecycleSignColdAdapterAuthorityV1::from_recovered_wal(",
                ),
            )
            require_tokens(
                adapter_path,
                "cold recovered signed-Broadcast reducer fast-forward",
                adapter_source,
                (
                    "fn advance_recovered_lifecycle_signed_broadcast(",
                    "verify_individual_signature(",
                    "let [reducer::Effect::Broadcast(message)] = core_effects.as_slice()",
                    "replayed != broadcast",
                    "next_reducer.pending_persistence_record().is_some()",
                    "next_reducer.awaiting_signature().is_some()",
                ),
            )
            require_literal_count(
                adapter_path,
                "cold recovered signed-Broadcast reducer fast-forward",
                adapter_source,
                '"Proposal cold replay requires its body and Prepare WAL successor"',
                2,
            )
            require_tokens(
                lifecycle_open_path,
                "cold recovered signed-Broadcast storage census",
                lifecycle_open_source,
                (
                    "PhaseBroadcast(",
                    "PhaseBroadcastAndNextSign(",
                    "ControlBroadcast(",
                    "assemble_storage_only_with_recovered_phase_broadcast_and_body_pipeline_startup",
                    "assemble_storage_only_with_recovered_phase_broadcast_and_next_sign_and_body_pipeline_startup",
                    "assemble_storage_only_with_recovered_control_broadcast_and_body_pipeline_startup",
                ),
            )
            require_tokens(
                ledger_path,
                "cold recovered phase Broadcast-and-Sign ledger join",
                ledger_source,
                (
                    "fn authenticate_recovered_phase_signed_broadcast_and_sign(",
                    "combined.broadcast_exactly_matches(&broadcast)",
                    "combined.exactly_matches_fresh_records(",
                    "fn revalidates_recovered_phase_signed_broadcast_and_sign(",
                ),
            )
            require_tokens(
                registry_path,
                "cold recovered phase Broadcast-and-Sign registry join",
                registry_source,
                (
                    "#[inline(never)] pub(in crate::sumeragi) fn prepare_cold_adapter_startup(",
                    "Self::prepare_cold_sign_branch(",
                    "Self::prepare_cold_signed_broadcast_branch(",
                    "#[inline(never)] fn prepare_cold_sign_branch(",
                    "#[inline(never)] fn prepare_cold_signed_broadcast_branch(",
                    "let pair_hint = matching.next()",
                    "if matching.next().is_some()",
                    "drop(matching)",
                    "Self::prepare_cold_signed_broadcast_and_next_vote_branch(",
                    "Self::prepare_cold_single_signed_broadcast_branch(",
                    "#[inline(never)] fn prepare_cold_single_signed_broadcast_branch(",
                    "#[inline(never)] fn prepare_cold_signed_broadcast_and_next_vote_branch(",
                    "authenticate_recovered_lifecycle_next_vote_body(&mut preview)",
                    "project_authenticated_cold_signed_broadcast_and_sign(verified, seal)",
                    "authenticate_recovered_phase_signed_broadcast_and_sign(",
                    "advance_recovered_lifecycle_signed_broadcast_and_sign(",
                    "#[inline(never)] pub(crate) fn install_recovered_wal_sign(",
                    "Self::install_recovered_sign_branch(",
                    "Self::install_recovered_broadcast_branch(",
                    "Self::install_recovered_broadcast_and_next_vote_branch(",
                    "#[inline(never)] fn install_recovered_sign_branch(",
                    "#[inline(never)] fn install_recovered_broadcast_branch(",
                    "#[inline(never)] fn install_recovered_broadcast_and_next_vote_branch(",
                    "fn install_recovered_broadcast_and_next_vote(",
                    "paired_next_sign: Some((next_sign_address, next_sign_digest))",
                    "fn phase_broadcast_and_next_vote_projection(",
                    "owns_recovered_phase_broadcast_and_next_sign(",
                ),
            )
            pair_install = _require_rust_item(
                registry_path,
                registry_source,
                "install_recovered_broadcast_and_next_vote",
                errors,
            )
            if pair_install is not None:
                require_tokens(
                    registry_path,
                    "cold recovered phase Broadcast-and-Sign registry join",
                    pair_install.source,
                    (
                        "paired_next_sign: Some((next_sign_address, next_sign_digest))",
                    ),
                )
            require_tokens(
                adapter_path,
                "cold recovered phase owner handoff",
                adapter_source,
                (
                    "#[inline(never)] fn authenticate_recovered_phase_vote_stage<'registry>(",
                    "Ok(Box::new(authenticated))",
                    "#[inline(never)] fn persist_recovered_phase_vote_stage<'registry>(",
                    "(*authenticated).persist_repair()",
                    "Ok(persisted)",
                    "#[inline(never)] fn prepare_recovered_phase_vote_cold_adapter_stage<'registry>(",
                    "local_proposal_attempt: Option<RecoveredLifecycleLocalProposalAttemptV1>",
                    "ProductionLifecycleAdapterStartupV1::recovered_with_local_proposal_attempt( adapter, effects, local_proposal_attempt, )",
                    "prepare_cold_adapter_startup(&verified, adapter_startup, body_store)",
                    "ColdPreparedStorageAuthenticatedRecoveredWalLifecycleStartup { adapter_startup, verified, persisted, }",
                    "#[inline(never)] fn install_recovered_phase_vote_sign_stage<'registry>(",
                    "(*prepared).install_recovered_sign()",
                    "#[inline(never)] fn open_recovered_phase_vote_seals_stage(",
                    "(*installed).open_production_owner_seals(",
                    "#[inline(never)] fn finish_recovered_phase_vote_owner_stage(",
                    "(*paired).into_owner(registry, payload_store, body_store)",
                ),
            )
            phase_branch = _require_rust_item(
                adapter_path,
                adapter_source,
                "open_recovered_phase_vote_branch",
                errors,
            )
            if phase_branch is not None:
                require_order(
                    adapter_path,
                    "cold recovered phase owner handoff",
                    phase_branch.source,
                    (
                        "Self::ensure_recovered_body_store_context(&body_store, &verified)",
                        "Self::open_recovered_non_apply_stores(",
                        "Self::authenticate_recovered_phase_vote_stage(",
                        "Self::persist_recovered_phase_vote_stage(authenticated)",
                        "Self::prepare_recovered_phase_vote_cold_adapter_stage( persisted, &body_store, local_proposal_attempt, )",
                        "Self::install_recovered_phase_vote_sign_stage(prepared)",
                        "Self::open_recovered_phase_vote_seals_stage(",
                        "Self::finish_recovered_phase_vote_owner_stage(",
                    ),
                )
            recovered_phase_broadcast_assembly = region(
                lifecycle_open_path,
                lifecycle_open_source,
                "cold recovered phase-Broadcast storage assembly",
                "fn assemble_storage_only_with_recovered_phase_broadcast_and_body_pipeline_startup(",
                "/// Assemble the exact standalone control Sign with every durable Fetch.",
            )
            require_tokens(
                lifecycle_open_path,
                "cold recovered phase-Broadcast storage assembly",
                recovered_phase_broadcast_assembly,
                (
                    "RecoveredWalStartupProjectionV1::PhaseBroadcast(projection, broadcast)",
                    "assemble_storage_only_with_terminal_validate_outcomes(",
                ),
            )
            recovered_control_broadcast_assembly = region(
                lifecycle_open_path,
                lifecycle_open_source,
                "cold recovered control-Broadcast storage assembly",
                "fn assemble_storage_only_with_recovered_control_broadcast_and_body_pipeline_startup(",
                "/// Assemble the standalone Decision Fetch with every durable body-backed Fetch.",
            )
            require_tokens(
                lifecycle_open_path,
                "cold recovered control-Broadcast storage assembly",
                recovered_control_broadcast_assembly,
                (
                    "RecoveredWalStartupProjectionV1::ControlBroadcast(control, broadcast)",
                    "assemble_storage_only_with_terminal_validate_outcomes(",
                ),
            )
            require_tokens(
                worker_path,
                "dedicated recovered Sign queue ownership",
                worker_source,
                (
                    "recovered_lifecycle_signs:",
                    "BTreeMap<RecoveredLifecycleSignDispatchKeyV1, V2IoTrackedRecoveredLifecycleSignV1>",
                    "fn transfer_recovered_lifecycle_sign_completion_at(",
                    "io.prepare_recovered_lifecycle_sign_completion(guarded, ownership_position)",
                    "fn recovered_lifecycle_signing_is_exact_and_class_sensitive_for_all_three_families()",
                    "fn recovered_lifecycle_sign_queue_retains_exact_owner_through_opaque_extraction()",
                    "fn recovered_lifecycle_sign_capacity_unavailable_leaves_no_dedicated_index()",
                ),
            )
            recovered_sign_capacity = region(
                worker_path,
                worker_source,
                "recovered Sign capacity capture release",
                "fn capture_recovered_lifecycle_sign_capacity<'a>(",
                "fn lifecycle_completion_worker_capacity(",
            )
            require_token_count(
                worker_path,
                "recovered Sign capacity capture release",
                recovered_sign_capacity,
                "operation.complete()",
                4,
            )
            reject_tokens(
                worker_path,
                "recovered Sign capacity capture release",
                recovered_sign_capacity,
                ("drop(operation)",),
            )
            rollback_unpublished = region(
                owner_path,
                owner_source,
                "unpublished recovered Sign claim rollback",
                "fn rollback_unpublished_turn(&mut self, lease: &TurnLease) -> bool {",
                "/// Rebuild records after seeding the ordinal high-water mark.",
            )
            require_tokens(
                owner_path,
                "unpublished recovered Sign claim rollback",
                rollback_unpublished,
                (
                    "lease.output_reservation.is_some()",
                    "assert!( inserted,",
                    "self.active_lease = None",
                ),
            )
            reject_tokens(
                owner_path,
                "unpublished recovered Sign claim rollback",
                rollback_unpublished,
                ("debug_assert!",),
            )
            require_tokens(
                owner_path,
                "unpublished recovered Sign rollback regression",
                owner_source,
                (
                    "fn unpublished_turn_rollback_restores_ready_and_clears_the_active_lease()",
                ),
            )
            launched_owner_fields = region(
                launch_path,
                launch_source,
                "launched unified lifecycle completion Drop order",
                "pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {",
                "/// Sole parked lifecycle completion owner for this height.",
            )
            require_order(
                launch_path,
                "launched unified lifecycle completion Drop order",
                launched_owner_fields,
                (
                    "services: ProductionV2Services",
                    "pending_lifecycle_completion: Option<PendingLifecycleCompletionV1>",
                    "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
                ),
            )
            recovered_fetch_dispatch = region(
                scheduler_path,
                scheduler_source,
                "lifecycle-owned recovered Decision Fetch dispatch",
                "fn dispatch_completion_with_runner_debt_and_required_ordinal(",
                "pub(super) fn dispatch_ready_validate_successor(",
            )
            require_order(
                scheduler_path,
                "lifecycle-owned recovered Decision Fetch dispatch",
                recovered_fetch_dispatch,
                (
                    "attest_ready_recovered_decision_fetch",
                    "authenticate_recovered_decision_fetch_request(",
                    "take_request_authority()",
                    "capture_lifecycle_completion_capacity_census(probes)",
                    "self.coordinator.plan_turn(inputs)",
                    "census.select_fetch(ordinal)",
                    "prepare_recovered_decision_fetch_request_registration(owner)",
                    "prepare_recovered_decision_fetch_dispatch",
                    "registration.commit(prepared, wait_source)",
                    "output.commit()",
                ),
            )
            require_tokens(
                scheduler_path,
                "lifecycle-owned recovered Decision Fetch dispatch",
                recovered_fetch_dispatch,
                (
                    "services.matches_lifecycle_body_store(body_store_identity)",
                    "services.matches_lifecycle_executor_output_guard(executor)",
                    "ReadyRecoveredDecisionFetchDemandV1::ExactOutputAndExecutor",
                    "LifecycleCompletionCapacityProbeV1::Fetch",
                    "authenticated_capacity(ordinal, &factory)",
                    "prepared.dispatch_key() != registration.dispatch_key()",
                    "installed != dispatch_key",
                ),
            )
            reject_tokens(
                scheduler_path,
                "sealed recovered Decision Fetch request dispatch",
                recovered_fetch_dispatch,
                (
                    "EffectWorkId",
                    "RuntimeEffectOwnership",
                    "PendingRuntimeEffectBinding",
                    "into_parts",
                    "settle",
                ),
            )
            recovered_fetch_phase_a = region(
                scheduler_path,
                scheduler_source,
                "recovered Decision Fetch response persistence Phase A",
                "fn persist_recovered_decision_fetch_response_after_runner(",
                "/// Plan, submit, and reblock one exact selected certified-Fetch response.",
            )
            require_order(
                scheduler_path,
                "recovered Decision Fetch response persistence Phase A",
                recovered_fetch_phase_a,
                (
                    "capture_lifecycle_capacity_rank(selector)",
                    "reservation.preflight_recovered_decision_fetch_target_absent()",
                    "executor.prepare_recovered_decision_fetch_body_persistence(prepared)",
                    "reservation.preflight_recovered_decision_fetch_body_persistence(&task)",
                    "executor.prepare_recovered_decision_fetch_response_claim(&task)",
                    "let mut next = self.coordinator.stage_durable_transaction()",
                    "next.plan_turn(inputs)",
                    "matches_claimed_dispatched_recovered_decision_fetch(",
                    "self.coordinator = next",
                    "claim.commit_with_queue(reservation, task)",
                    "assert_eq!(self.coordinator.active_lease.as_ref(), Some(&lease))",
                ),
            )
            recovered_fetch_ingress = region(
                turn_driver_path,
                turn_driver_source,
                "unified recovered Decision Fetch ingress driver",
                "pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(",
                "fn drive_recovered_ingress_selector<'cursor>(",
            )
            require_order(
                turn_driver_path,
                "unified recovered Decision Fetch ingress driver",
                recovered_fetch_ingress,
                (
                    "if !self.runner_turn_matches(",
                    "LifecycleRunnerRankTarget::Ingress",
                    "return ProductionLifecycleIngressTurnV1::PassThrough(runner)",
                    "self.drive_recovered_ingress_selector(selector, runner)",
                ),
            )
            recovered_fetch_ingress_handoff = region(
                turn_driver_path,
                turn_driver_source,
                "validated recovered Decision Fetch Phase-A handoff",
                "fn drive_recovered_ingress_selector<'cursor>(",
                "fn settle_parked_recovered_sign_completion(",
            )
            require_order(
                turn_driver_path,
                "validated recovered Decision Fetch Phase-A handoff",
                recovered_fetch_ingress_handoff,
                (
                    "persist_recovered_decision_fetch_response_after_runner(",
                    "drop(runner)",
                    "ProductionLifecycleIngressTurnV1::Selected(selected)",
                ),
            )
            require_tokens(
                launch_path,
                "recovered Decision Fetch source-order regression",
                launch_source,
                (
                    "fn recovered_decision_fetch_phase_a_is_reachable_only_after_runner_validation()",
                ),
            )
            recovered_fetch_ready = region(
                registry_validate_path,
                registry_validate_source,
                "closed Ready and claimed recovered Decision Fetch carrier",
                "pub(super) fn attest_ready_recovered_decision_fetch(",
                "/// Project a comparison-only seal for this exact registry instance.",
            )
            require_tokens(
                registry_validate_path,
                "closed Ready and claimed recovered Decision Fetch carrier",
                recovered_fetch_ready,
                (
                    "fetch.dispatch_key.is_some()",
                    "fetch.matches_current_ready_record(address, digest, coordinator)",
                    "RecoveredDecisionFetchDispatchIdentityV1::new(",
                    "project_recovered_decision_fetch_request(identity)",
                    "fn matches_claimed_dispatched_recovered_decision_fetch(",
                    "fetch.dispatch_key == Some(key)",
                    "fetch.matches_claimed_record(address, digest, coordinator, lease)",
                    "fn prepare_recovered_decision_fetch_dispatch(",
                ),
            )
            recovered_fetch_projection = region(
                wal_recovery_path,
                wal_recovery_source,
                "payload-free recovered Decision Fetch projection",
                "pub(super) fn project_recovered_decision_fetch_request(",
                "/// Prove the authenticated recovery cut retains this exact Fetch.",
            )
            require_tokens(
                wal_recovery_path,
                "payload-free recovered Decision Fetch projection",
                recovered_fetch_projection,
                (
                    "AdapterEffect::FetchBody {",
                    "manifest: None",
                    "certificate: Some(certificate)",
                    "RecoveredDecisionFetchRequestAuthorityV1::from_registry_projection(",
                ),
            )
            reject_tokens(
                wal_recovery_path,
                "payload-free recovered Decision Fetch projection",
                recovered_fetch_projection,
                ("EffectWorkId", "RuntimeEffectOwnership", "into_parts"),
            )
            recovered_fetch_registration = region(
                effects_path,
                effects_source,
                "dedicated recovered Decision Fetch request owner census",
                "pub(in crate::sumeragi) fn recovered_decision_fetch_registration_available(",
                "/// Take ownership of an exact-body store opened during sealed preflight.",
            )
            require_tokens(
                effects_path,
                "dedicated recovered Decision Fetch request owner census",
                recovered_fetch_registration,
                (
                    "self.validated_certified_request_presence().is_err()",
                    "self.outstanding_requests.len().checked_add(self.recovered_decision_fetches.len())",
                    "owner.conflicts_with_ordinary_tracker(&self.outstanding_requests)",
                    "owner.matches_body_coordinates(pending.task.round, pending.task.subject)",
                    "pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_request_registration(",
                    "PreparedRecoveredDecisionFetchRequestRegistrationV1 { executor: self, owner: Some(owner), }",
                ),
            )
            require_tokens(
                effects_path,
                "complete recovered Decision Fetch request census and terminal fence",
                effects_source,
                (
                    "recovered_decision_fetches: BTreeMap<",
                    "recovered_decision_fetch_by_request: BTreeMap<",
                    "fn recovered_decision_fetch_request_index_is_exact_and_empty(&self) -> bool",
                    "self.recovered_decision_fetch_request_index_is_exact_and_empty()",
                    "fn validated_certified_request_presence(",
                    "Ok(!pending_hashes.is_empty() || !recovered_hashes.is_empty())",
                ),
            )
            ordinary_fetch_admission = region(
                effects_path,
                effects_source,
                "ordinary and recovered Decision Fetch coordinate fence",
                "fn begin_fetch<S: V2EffectServices>(",
                "fn retained_body_manifest_hash(",
            )
            require_tokens(
                effects_path,
                "ordinary and recovered Decision Fetch coordinate fence",
                ordinary_fetch_admission,
                (
                    "self.recovered_decision_fetches.values()",
                    "owner.matches_body_coordinates(round, subject)",
                ),
            )
            require_literal_count(
                effects_path,
                "ordinary and recovered Decision Fetch coordinate fence",
                ordinary_fetch_admission,
                '"body-fetch coordinates already have a recovered Decision Fetch owner"',
                1,
            )
            require_tokens(
                effects_path,
                "symmetric recovered Decision Fetch owner census",
                effects_source,
                (
                    "owner.matches_body_coordinates(pending.task.round, pending.task.subject)",
                    "fn recovered_decision_fetch_fences_later_ordinary_body_coordinates()",
                    "executor.validated_certified_request_presence()",
                ),
            )
            recovered_fetch_selector = region(
                selector_path,
                selector_source,
                "typed recovered Decision Fetch selector consumption",
                "pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_body_persistence(",
                "/// Consume one exact selected family into a bounded body-store command.",
            )
            require_order(
                selector_path,
                "typed recovered Decision Fetch selector consumption",
                recovered_fetch_selector,
                (
                    "self.revalidate_recovered_decision_fetch_response_candidate(",
                    "PreparedCertifiedResponseCandidate::Recovered(candidate)",
                    "let authenticated = candidate.into_authenticated_response()",
                    "RecoveredDecisionFetchBodyPersistenceTaskV1 {",
                ),
            )
            require_tokens(
                selector_path,
                "typed recovered Decision Fetch selector target",
                selector_source,
                (
                    "PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence",
                    "LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence",
                    "fn matches_recovered_decision_fetch_key(",
                ),
            )
            require_tokens(
                worker_path,
                "typed recovered Decision Fetch selector target consumer",
                worker_source,
                (
                    "target.matches_recovered_decision_fetch_key(task.dispatch_key())",
                ),
            )
            recovered_fetch_next_selector = region(
                selector_path,
                selector_source,
                "queue-owned recovered Decision Fetch selector",
                "pub(crate) fn prepare_next_recovered_decision_fetch_ingress_selector(",
                "/// Classify the exact selected certified-response occurrence without mutation.",
            )
            require_order(
                selector_path,
                "queue-owned recovered Decision Fetch selector",
                recovered_fetch_next_selector,
                (
                    "self.lifecycle_terminal_subject()",
                    "capture_next_lifecycle_queue_cut(",
                    "v2_ingress_head_can_drain(occurrence.inbound(), self, terminal_subject)",
                    "self.capture_lifecycle_ingress_selector(cut)",
                    "prepared.queue_witness.selected_disposition()",
                    "PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence",
                    ".selected_claimed_response_family()",
                ),
            )
            reject_tokens(
                selector_path,
                "queue-owned recovered Decision Fetch selector",
                recovered_fetch_next_selector,
                (
                    "target_physical_ordinal:",
                    "prepare_lifecycle_ingress_selector(",
                    "try_recv",
                    "commit_exact_dequeue",
                ),
            )
            recovered_fetch_queue_cut = region(
                ingress_position_path,
                ingress_position_source,
                "queue-owned recovered Decision Fetch fair cut",
                "pub(super) fn capture_next_lifecycle_queue_cut(",
                "fn capture_lifecycle_queue_cut_for(",
            )
            require_tokens(
                ingress_position_path,
                "queue-owned recovered Decision Fetch fair cut",
                recovered_fetch_queue_cut,
                (
                    "LifecycleQueueCutTarget::NextAdmissible",
                    "predicate: impl FnMut(&FairIngressSelectorOccurrence) -> bool",
                    "Result<Option<FairIngressQueueCut<'_>>, FairIngressQueueCutError>",
                ),
            )
            recovered_fetch_fair_selection = region(
                ingress_position_path,
                ingress_position_source,
                "queue-owned recovered Decision Fetch fair selection",
                "fn select_next_admissible_ordinal(",
                "fn mint_pending_identities(",
            )
            require_order(
                ingress_position_path,
                "queue-owned recovered Decision Fetch fair selection",
                recovered_fetch_fair_selection,
                (
                    "geometry.ready_prefix.iter()",
                    "selector.queue_gate() != occurrence.value.queue_gate",
                    "select_fair_v2_ingress_candidate(",
                    "occurrence.physical_admission_ordinal()",
                    "occurrence.queue_gate()",
                    "occurrence.is_obsolete()",
                    "predicate(occurrence)",
                ),
            )
            reject_tokens(
                ingress_position_path,
                "queue-owned recovered Decision Fetch fair selection",
                recovered_fetch_fair_selection,
                ("pop_", "remove(", "rotate_", "dequeue_selected_locked"),
            )
            shared_fair_selection = region(
                sumeragi_path,
                sumeragi_source,
                "shared strict-then-dependency fair selection",
                "fn select_fair_v2_ingress_candidate<T>(",
                "/// Fixed-capacity, roster-aware v2 ingress with per-hop admission and service fairness.",
            )
            require_order(
                sumeragi_path,
                "shared strict-then-dependency fair selection",
                shared_fair_selection,
                (
                    "for dependency_pass in [false, true]",
                    "for (source_index, source_candidates) in candidates.iter().enumerate()",
                    "for candidate in source_candidates",
                    "gate == FairV2IngressQueueGateVerdict::Blocked",
                    "dependency != dependency_pass",
                    "obsolete || predicate(candidate)",
                    "return Some((source_index, ordinal, disposition))",
                ),
            )
            ordinary_fair_dequeue = region(
                sumeragi_path,
                sumeragi_source,
                "ordinary shared fair selection call",
                "fn try_recv_if_at_checked_classified(",
                "/// Commit one already selected occurrence",
            )
            require_tokens(
                sumeragi_path,
                "ordinary shared fair selection call",
                ordinary_fair_dequeue,
                ("select_fair_v2_ingress_candidate(",),
            )
            require_tokens(
                effects_path,
                "shared pure ingress drain predicate",
                effects_source,
                (
                    "fn v2_ingress_head_can_drain<R: EffectRuntime>(",
                    "certified_body_request_is_superseded_after_decision(",
                    "executor.can_admit_network_message_with_ingress_ownership(",
                ),
            )
            require_tokens(
                turn_driver_path,
                "ordinary runner shared ingress drain predicate",
                turn_driver_source,
                ("v2_ingress_head_can_drain(occurrence.inbound(), executor, terminal_subject,)",),
            )
            require_tokens(
                effects_path,
                "queue-owned recovered Decision Fetch selector behavior",
                effects_source,
                (
                    "fn recovered_decision_fetch_fences_later_ordinary_body_coordinates()",
                    ".prepare_next_recovered_decision_fetch_ingress_selector(&ingress)",
                ),
            )
            for literal in (
                '"a later recovered response cannot leapfrog the ordinary fair winner"',
                '"the queue-owned selector chooses the next fair exact family occurrence"',
                '"queue-owned selector discovery cannot dequeue or renumber ingress"',
            ):
                require_literal_count(
                    effects_path,
                    "queue-owned recovered Decision Fetch selector behavior",
                    effects_source,
                    literal,
                    1,
                )
            recovered_fetch_claim = region(
                effects_path,
                effects_source,
                "recovered Decision Fetch response claim publication",
                "pub(in crate::sumeragi) fn commit_with_queue(",
                "impl RecoveredDecisionFetchResponseCandidateV1",
            )
            require_order(
                effects_path,
                "recovered Decision Fetch response claim publication",
                recovered_fetch_claim,
                (
                    "owner.matches_response_claim_preflight(response_hash, preflight)",
                    "owner.commit_exact_response_claim(response_hash)",
                    "queue.commit_recovered_decision_fetch_body_persistence(task)",
                ),
            )
            recovered_fetch_mixed_head = region(
                worker_path,
                worker_source,
                "recovered Decision Fetch mixed completion head fence",
                "fn take_io_completion(&mut self, runtime_capacity_available: bool)",
                "fn take_recovered_lifecycle_sign_completion(",
            )
            require_order(
                worker_path,
                "recovered Decision Fetch mixed completion head fence",
                recovered_fetch_mixed_head,
                (
                    "let ownership_position =",
                    "io.completion_ownership_at(ownership_position)",
                    "owned.recovered_decision_fetch.is_some()",
                    "return IoCompletionTake::retained_runtime()",
                    "io.try_recv_completion_unacknowledged()",
                ),
            )
            recovered_fetch_classifier = region(
                worker_path,
                worker_source,
                "unified recovered Decision Fetch completion classifier",
                "pub(in crate::sumeragi) fn take_next_lifecycle_completion(",
                "/// Drain only the oldest recovered-Sign guard;",
            )
            require_order(
                worker_path,
                "unified recovered Decision Fetch completion classifier",
                recovered_fetch_classifier,
                (
                    "V2IoCompletion::RecoveredDecisionFetchBodyPersisted(guarded)",
                    "prepare_recovered_decision_fetch_body_completion(guarded, 0)",
                    "LifecycleCompletionTakeV1::DecisionFetch(",
                ),
            )
            require_tokens(
                worker_path,
                "unified recovered Decision Fetch worker ownership",
                worker_source,
                (
                    "PersistRecoveredDecisionFetchBody(RecoveredDecisionFetchBodyPersistenceTaskV1)",
                    "recovered_decision_fetch_bodies: BTreeMap<RecoveredDecisionFetchDispatchKeyV1, V2IoTrackedRecoveredDecisionFetchBodyV1>",
                    "V2IoCompletion::RecoveredDecisionFetchBodyPersisted",
                    "V2IoCompletionAcknowledgement::RecoveredDecisionFetchRetained",
                    "fn take_next_lifecycle_completion(",
                    "fn recovered_decision_fetch_queue_transitions_and_parks_until_dedicated_extraction()",
                ),
            )
            parked_fetch_completion = region(
                worker_path,
                worker_source,
                "opaque parked recovered Decision Fetch completion",
                "pub(in crate::sumeragi) struct PreparedRecoveredDecisionFetchBodyCompletionV1 {",
                "impl PreparedRecoveredLifecycleSignCompletionV1",
            )
            reject_tokens(
                worker_path,
                "opaque parked recovered Decision Fetch completion",
                parked_fetch_completion,
                (
                    "fn into_parts(",
                    "fn durable_receipt(",
                    "fn response(",
                    "fn acknowledge(",
                    "fn settle(",
                ),
            )
            single_store_prepare = region(
                registry_path,
                registry_source,
                "shared-ordinal recovered Fetch-to-Store preparation",
                "pub(super) fn prepare_recovered_decision_fetch_store_successor",
                "impl<'registry, 'adapter> PreparedRecoveredDecisionFetchStoreSuccessor<'registry, 'adapter>",
            )
            reject_tokens(
                registry_path,
                "shared-ordinal recovered Fetch-to-Store preparation",
                single_store_prepare,
                ("coordinator.high_water", ".checked_add(1)", "store_address"),
            )
            single_store_bind = region(
                registry_path,
                registry_source,
                "staged recovered Store registry binding",
                "impl<'registry, 'adapter> PreparedRecoveredDecisionFetchStoreSuccessor<'registry, 'adapter>",
                "impl<'adapter> BoundRecoveredDecisionFetchStoreSuccessor<'_, 'adapter>",
            )
            require_order(
                registry_path,
                "staged recovered Store registry binding",
                single_store_bind,
                (
                    "exact_staged_recovered_decision_store_address(",
                    "self.registry.entries.contains_key(&store_address)",
                    "pub(super) fn exact_staged_recovered_decision_store_address(",
                    "coordinator.records.get(&child_ordinal)",
                    "ConcreteWorkAddress::new(record.owner, child_ordinal, child_slot)",
                    "store.matches_current_ready_record(",
                ),
            )
            single_store_transition = region(
                body_pipeline_path,
                body_pipeline_source,
                "staged recovered Fetch-to-Store transition",
                "pub(super) fn prepare_recovered_decision_fetch_store_transition",
                "/// Stage one recovered Sign",
            )
            require_order(
                body_pipeline_path,
                "staged recovered Fetch-to-Store transition",
                single_store_transition,
                (
                    "stage_recovered_decision_fetch_store_transition(",
                    ".bind_staged_child(",
                ),
            )
            require_tokens(
                body_pipeline_path,
                "staged recovered Fetch-to-Store transition",
                single_store_transition,
                ("transition.child_ordinal",),
            )
            recovered_fetch_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Decision Fetch-to-Store settlement",
                "pub(in crate::sumeragi) fn settle_recovered_decision_fetch_store(",
                "/// Reserve, claim, and queue one recovered Sign",
            )
            require_order(
                launch_path,
                "restart-closed recovered Decision Fetch-to-Store settlement",
                recovered_fetch_settlement,
                (
                    "prepare_lifecycle_ingress_selector(",
                    "prepare_recovered_decision_fetch_owner_retirement(",
                    "into_locked_recovered_decision_fetch_dequeue(",
                    "prepare_recovered_decision_fetch_store_adapter_authority(",
                    "prepare_recovered_decision_fetch_store_adapter(",
                    "prepare_recovered_decision_fetch_store_successor(",
                    "prepare_recovered_decision_fetch_store_transition(",
                    "begin_fail_stop_operation()",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "commit_recovered_decision_fetch_owner_retirement(retirement)",
                    "locked_dequeue.commit()",
                    "completion.acknowledge_after_publication()",
                    "operation.complete()",
                ),
            )
            require_tokens(
                launch_path,
                "restart-closed recovered Decision Fetch-to-Store settlement",
                recovered_fetch_settlement,
                (
                    "*pending_lifecycle_completion = Some(PendingLifecycleCompletionV1::RecoveredDecisionFetch(completion),)",
                    "owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure)",
                    "ProductionRecoveredDecisionFetchStoreSettlementV1::RestartRequired",
                    "ProductionRecoveredDecisionFetchStoreSettlementV1::Applied",
                ),
            )
            reject_tokens(
                launch_path,
                "dedicated recovered Decision Fetch-to-Store settlement",
                recovered_fetch_settlement,
                ("EffectWorkId", "RuntimeEffectOwnership", "into_parts"),
            )
            require_tokens(
                worker_path,
                "recovered Decision Fetch worker acknowledgement tail",
                region(
                    worker_path,
                    worker_source,
                    "recovered Decision Fetch worker acknowledgement tail",
                    "fn acknowledge_recovered_decision_fetch_body(",
                    "fn prepare_certified_fetch_body_persistence_ack(",
                ),
                (
                    "fn acknowledge_recovered_decision_fetch_body(",
                    ".recovered_decision_fetch_bodies",
                    ".remove(&key)",
                ),
            )
            require_tokens(
                worker_path,
                "recovered Decision Fetch guarded acknowledgement tail",
                worker_source,
                (
                    "fn acknowledge_after_publication(mut self)",
                    "self.drop_guard.disarm()",
                ),
            )
            require_tokens(
                ledger_path,
                "recovered Decision Store cold restart and marker-prefix closure",
                ledger_source,
                (
                    "fn authenticate_recovered_decision_fetch_store(",
                    "fn open_recovered_decision_store_startup(",
                    "fn stage_recovered_decision_apply_projection(",
                    "successor_records_after_live_store(",
                    "fn recovered_decision_store_crash_prefix_restarts_once_then_stutters()",
                    "fn recovered_decision_store_restart_rejects_an_exact_child_key_collision()",
                ),
            )
            require_tokens(
                body_pipeline_path,
                "recovered Decision Fetch payload-free parent transition",
                body_pipeline_source,
                (
                    "fn stage_recovered_decision_fetch_store_transition(",
                    "DurablePayloadReference::None",
                    "DurableContinuationEdge::FetchToStore",
                    "BodyStagePayloadRelationV1::RecoveredDecisionFetch",
                    "fn persist_exact_successor(",
                    "fn commit_after_publication(self)",
                ),
            )
            require_tokens(
                adapter_path,
                "recovered Decision Store cold adapter reconstruction",
                adapter_source,
                (
                    "fn advance_recovered_decision_fetch_store(",
                    "project_store_adapter_authority(body)",
                    "project_decision_fetch_store(verified, projection_body, preview.store_effect())",
                    "preview.commit_after_durable_settlement()",
                ),
            )
            require_tokens(
                body_store_path,
                "recovered Decision Store body-frame reconstruction",
                body_store_source,
                (
                    "struct RecoveredDecisionFetchStoreBodyAuthorityV1",
                    "fn recovered_decision_fetch_store_body(",
                    "Ok(RecoveredDecisionFetchStoreBodyAuthorityV1 { manifest: manifest.clone(), durable: durable.clone(), })",
                ),
            )
            require_tokens(
                lifecycle_open_path,
                "typed recovered Decision Store storage census",
                lifecycle_open_source,
                (
                    "RecoveredWalStartupProjectionV1::DecisionStore",
                    "assemble_storage_only_with_recovered_decision_store_and_body_pipeline_startup",
                    "recovered_decision_store_chain_records(",
                ),
            )
            require_tokens(
                registry_validate_path,
                "dedicated recovered Decision Store registry install",
                registry_validate_source,
                (
                    "RecoveredWalRegistrySlotV1::DecisionStore",
                    "fn install_recovered_wal_decision_store<'registry>(",
                    "ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore",
                ),
            )
            require_order(
                launch_path,
                "launched unified lifecycle completion/capacity Drop order",
                launched_owner_fields,
                (
                    "services: ProductionV2Services",
                    "pending_lifecycle_completion: Option<PendingLifecycleCompletionV1>",
                    "pending_ingress_capacity: Option<PendingIngressCapacityV1>",
                    "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
                ),
            )
            request_scoped_response = region(
                transport_path,
                transport_source,
                "request-scoped certified response authentication",
                "pub(in crate::sumeragi) fn authenticate_response(",
                "/// Certified-body response admitted for one outstanding exact request.",
            )
            require_tokens(
                transport_path,
                "request-scoped certified response authentication",
                request_scoped_response,
                (
                    "authenticate_certified_body_response_for_request(",
                    "response.validate_against(",
                    "verify_signature(",
                    "decode_framed_signed_block(&response.body)",
                    "AuthenticatedCertifiedBodyResponse { response }",
                ),
            )
            require_tokens(
                kura_path,
                "process-local Kura identity seal",
                kura_source,
                (
                    "instance_identity: Arc<KuraInstanceIdentityMarker>",
                    "struct KuraInstanceIdentity(Arc<KuraInstanceIdentityMarker>)",
                    "Arc::ptr_eq(&self.0, &kura.instance_identity)",
                    "Arc::ptr_eq(&self.0, &other.0)",
                    "fn instance_identity(&self) -> KuraInstanceIdentity",
                    "fn instance_identity_names_only_the_exact_live_kura()",
                    "store_root_directory: BoundProgressDirectory",
                    "Self::open_safety_wal_store_root_directory(&store_root, &store_root_lock_file)?",
                ),
            )
