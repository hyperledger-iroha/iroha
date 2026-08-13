# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

def test_successor_run_inner_parser_rejects_neighbor_lookalike(
    tmp_path: Path,
) -> None:
    """Successor checks may consume only the parsed `run_inner` item."""

    module = load_checker()
    for relative in (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "crates/iroha_core/src/sumeragi/mod.rs",
        "crates/iroha_core/src/sumeragi/status.rs",
        "crates/iroha_core/src/sumeragi/v2_first_release_recovery.rs",
        "crates/iroha_core/src/sumeragi/v2.rs",
        "crates/iroha_core/src/sumeragi/v2_runtime.rs",
        "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "crates/iroha_core/src/sumeragi/v2_context.rs", "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "crates/iroha_core/src/sumeragi/v2_body_store.rs",
        "crates/iroha_core/src/sumeragi/safety_wal.rs",
        "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "crates/iroha_core/src/sumeragi/v2_transport.rs",
        "crates/iroha_core/src/kura.rs",
        "scripts/run_sumeragi_v2_release_gates.sh",
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    copy_reviewed_rust_include_components(tmp_path)

    runner = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner.rs"
    source = runner.read_text(encoding="utf-8")
    run_inner_items = module.rust_items(source, "run_inner")
    assert len(run_inner_items) == 1
    run_inner = run_inner_items[0]
    owner_binding = (
        "let mut pending_successor_activation = recovered_successor_activation\n"
        "        .map(|authority| PendingSuccessorActivation::recovered(authority, &common_config.key_pair))\n"
        "        .transpose()?;"
    )
    assert run_inner.source.count(owner_binding) == 1
    weakened = run_inner.source.replace(
        owner_binding,
        "let mut pending_successor_activation = None;",
        1,
    )
    neighboring_lookalike = (
        "\n\nfn parser_only_run_inner_lookalike() {\n"
        f"    {owner_binding}\n"
        "    let _ = &mut pending_successor_activation;\n"
        "}\n"
    )
    assert source.count(run_inner.source) == 1
    runner.write_text(
        source.replace(
            run_inner.source,
            weakened + neighboring_lookalike,
            1,
        ),
        encoding="utf-8",
    )

    errors = module._successor_production_source_fidelity_errors(tmp_path)
    assert any(
        "run_inner recovery ownership omits production refinement tokens"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative_path", "region_marker", "old", "new", "error_fragment"),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
            "fn dispatch_recovered_lifecycle_sign_with_runner_debt(",
            "services.matches_lifecycle_body_store(body_store_identity)",
            "true",
            "lifecycle-owned recovered Sign dispatch must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
            "fn dispatch_recovered_lifecycle_sign_with_runner_debt(",
            "reservation.class() == CapacityClass::Consensus",
            "true",
            "lifecycle-owned recovered Sign dispatch must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "impl PreparedRecoveredLifecycleSignCompletionV1",
            "result.is_exact()",
            "true",
            "adapter-private recovered Sign completion projection omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion(",
            "verify_individual_signature(",
            "trust_individual_signature(",
            "drop-inert recovered Sign adapter preview must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion(",
            "vote.phase == wire::GlobalPhase::Prepare",
            "true",
            "closed recovered Sign adapter successor shapes omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) fn settle_recovered_lifecycle_sign_broadcast(",
            "output_guard.begin_fail_stop_operation()",
            "output_guard.is_open()",
            "restart-closed recovered Sign-to-Broadcast settlement must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) fn settle_recovered_lifecycle_sign_broadcast(",
            "transition.persist_exact_successor().is_err()",
            "transition.skip_durable_publication().is_err()",
            "restart-closed recovered Sign-to-Broadcast settlement must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
            "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
            "services.matches_lifecycle_body_store(body_store_identity)",
            "true",
            "restart-safe recovered signed-Broadcast refanout must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
            "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
            "settle_turn(lease, super::TurnOutcome::Blocked(wait))",
            "settle_turn(lease, super::TurnOutcome::Terminal(TerminalOutcome::Completed(None)))",
            "restart-safe recovered signed-Broadcast refanout must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
            "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
            "recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal",
            "recovered_lifecycle_signed_broadcast_unchecked_adjacent_ordinal",
            "restart-safe recovered signed-Broadcast refanout must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
            "fn recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(",
            "next.ordinal == broadcast_ordinal.checked_add(1)?",
            "next.ordinal > broadcast_ordinal",
            "retained recovered Broadcast-and-next-Vote pair seal omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
            "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
            "for ready_ordinal in &exact_ready",
            "for ready_ordinal in core::iter::once(&ordinal)",
            "restart-safe recovered signed-Broadcast refanout must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn capture_recovered_lifecycle_signed_broadcast_refanout(",
            "authority.consume_for_service(RecoveredLifecycleSignBroadcastOutputPermitV1::new())",
            "authority.into_parts()",
            "durable recovered signed-Broadcast service capture omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn capture_recovered_lifecycle_cold_proposal_message(",
            "pending.prepare_atomic_fanout_batch(fanouts)",
            "Ok(None)",
            "durable recovered signed-Broadcast service capture omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs",
            "fn recover_durable_signed_broadcast(",
            "verified.verify_consensus_message(message)",
            "Ok(())",
            "cold recovered signed-Broadcast WAL and roster join omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "fn advance_recovered_lifecycle_signed_broadcast(",
            "let [reducer::Effect::Broadcast(message)] = core_effects.as_slice()",
            "let [message, ..] = core_effects.as_slice()",
            "cold recovered signed-Broadcast reducer fast-forward omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs",
            "fn assemble_storage_only_with_recovered_phase_broadcast_and_durable_fetch_startup(",
            "RecoveredWalStartupProjectionV1::PhaseBroadcast(projection, broadcast)",
            "RecoveredWalStartupProjectionV1::PhaseVote(projection)",
            "cold recovered phase-Broadcast storage assembly omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
            "fn authenticate_recovered_phase_signed_broadcast_and_sign(",
            "combined.broadcast_exactly_matches(&broadcast)",
            "true",
            "cold recovered phase Broadcast-and-Sign ledger join omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
            "fn prepare_cold_adapter_startup(",
            "authenticate_recovered_lifecycle_next_vote_body(&mut preview)",
            "authenticate_recovered_lifecycle_next_vote_body_unchecked(&mut preview)",
            "cold recovered phase Broadcast-and-Sign registry join omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
            "fn install_recovered_broadcast_and_next_vote(",
            "paired_next_sign: Some((next_sign_address, next_sign_digest))",
            "paired_next_sign: None",
            "cold recovered phase Broadcast-and-Sign registry join omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs",
            "fn assemble_storage_only_with_recovered_phase_broadcast_and_next_sign_and_durable_fetch_startup(",
            "RecoveredWalStartupProjectionV1::PhaseBroadcastAndNextSign(",
            "RecoveredWalStartupProjectionV1::PhaseBroadcast(",
            "cold recovered signed-Broadcast storage census omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "fn install_recovered_sign(",
            "prepare_cold_adapter_startup(&verified, adapter_startup, body_store)",
            "prepare_cold_adapter_startup_unchecked(&verified, adapter_startup, body_store)",
            "cold recovered phase owner handoff omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn prepare_recovered_lifecycle_sign_completion_with_body<'executor>(",
            ".prepare_recovered_lifecycle_sign_completion_with_body(permit, completion)",
            ".prepare_recovered_lifecycle_sign_completion(completion)",
            "single-preview recovered next-Vote body service join must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "fn production_recovered_proposal_sign_joins_exact_next_vote_body_store()",
            "fn production_recovered_proposal_sign_joins_exact_next_vote_body_store()",
            "fn production_recovered_proposal_sign_skips_next_vote_body_store()",
            "recovered Sign adapter preview behavior regression omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "fn project_proposal_exact_output_authority(",
            "self.shape() != RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign",
            "false",
            "affine recovered Proposal exact-output projection must remain shape closed",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn capture_recovered_lifecycle_proposal_exact_output(",
            "if self.proposal_work_retired",
            "if false",
            "recovered Proposal output must remain terminal after Decision",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn capture_recovered_lifecycle_proposal_exact_output(",
            "identity.same_instance(&body_store_identity)",
            "true",
            "recovered Proposal exact-output capture must retain its body-store owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn capture_recovered_lifecycle_proposal_exact_output(",
            "Arc::ptr_eq(&self.output_guard, &authority_output_guard)",
            "true",
            "recovered Proposal exact-output capture must retain its output guard",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn capture_recovered_lifecycle_proposal_exact_output(",
            "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(\n                retry_authority,\n            )",
            "RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(unreachable!())",
            "recovered Proposal capacity retry must remain source-token guarded",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn prepare_atomic_fanout_batch(",
            "if !self.ownership_capacity_available(&additions)?",
            "if false",
            "atomic Proposal fanout preflight must preserve aggregate capacity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn prepare_atomic_fanout_batch(",
            "aggregate.checked_add(count)",
            "aggregate.saturating_add(count)",
            "atomic Proposal fanout preflight must preserve aggregate capacity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn capture_recovered_lifecycle_proposal_exact_output(",
            "proposal\n            .validate(&self.context)",
            "Ok::<(), String>(())\n            .map_err(|error| error.to_string())",
            "retry-safe recovered Proposal exact-output capture omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn broadcast_consensus(",
            "self.enqueue_atomic_fanout_batch_while_guarded(",
            "self.enqueue_exact_fanout_while_guarded(",
            "live Proposal output must not split control from chunk ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound()",
            "fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound()",
            "fn recovered_proposal_exact_output_allows_partial_control()",
            "atomic Proposal output behavior regression omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn atomic_fanout_batch_preflights_aggregate_capacity_and_rebases_only_on_commit()",
            "fn atomic_fanout_batch_preflights_aggregate_capacity_and_rebases_only_on_commit()",
            "fn atomic_fanout_batch_allows_one_child_prefix()",
            "atomic Proposal aggregate-capacity regression omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn authenticate_recovered_lifecycle_next_vote_body_catalogs(",
            "durable_bodies.get(&key) != Some(durable)",
            "false",
            "exact recovered next-Vote body catalog join omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "fn consume_for_adapter(",
            "body_store_identity.same_instance(expected_body_store_identity)",
            "true",
            "opaque recovered next-Vote body authority omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "fn project_broadcast_and_sign_authority(",
            "self.adapter.authenticate_recovered_lifecycle_next_vote(",
            "self.adapter.trust_recovered_lifecycle_next_vote(",
            "affine recovered Broadcast-and-next-Sign adapter projection must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs",
            "fn into_candidate_projection(",
            "self.wal_identity.is_exact()",
            "true",
            "full executable recovered next-WAL-Vote candidate must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs",
            "fn project_cold_adapter_replay_authority(",
            "self.cold_adapter_authority_minted = true",
            "self.cold_adapter_authority_minted = false",
            "affine recovered Broadcast-and-next-Sign cold adapter projection must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs",
            "fn owns_spliced_candidates(",
            "candidates.get(&self.broadcast.candidate.key) == Some(&self.broadcast.candidate)",
            "true",
            "combined cold census must retain the exact Broadcast without claiming unrelated carriers",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs",
            "fn project_cold_adapter_next_sign(",
            "self.is_exact(verified)",
            "true",
            "sealed recovered next-WAL-Vote cold adapter projection must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "fn advance_recovered_lifecycle_signed_broadcast_and_sign(",
            "verified.verify_consensus_message(message)",
            "Ok::<(), AdapterError>(())",
            "recovered Broadcast-and-next-Sign cold adapter replay must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "impl RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1",
            "wire::GlobalPhase::Commit => tag.view() >= next_vote.round.view",
            "wire::GlobalPhase::Commit => tag.view() == next_vote.round.view",
            "opaque recovered Broadcast-and-next-Sign cold adapter authority omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "fn advance_recovered_lifecycle_signed_broadcast_and_sign(",
            "replayed_next_sign != next_sign",
            "false",
            "recovered Broadcast-and-next-Sign cold adapter replay must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
            "fn project_validated_recovered_lifecycle_signed_broadcast_and_sign_at(",
            "let next_sign_ordinal = broadcast_ordinal.checked_add(1)?",
            "let next_sign_ordinal = broadcast_ordinal.checked_add(2)?",
            "frame-bound recovered Broadcast-and-next-Sign ledger classifier omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
            "fn recovered_lifecycle_signed_broadcast_and_sign_pairs(",
            "&index,",
            "&RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1::new(&self.records),",
            "combined Broadcast-and-next-Sign enumeration must reuse one bounded frame index",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
            "fn exactly_matches_ledger(&self, ledger: &LifecycleLedgerV1) -> bool {",
            "project_recovered_lifecycle_signed_broadcast_and_sign_at(self.broadcast_ordinal)",
            "project_recovered_lifecycle_signed_broadcast_and_sign_at(0)",
            "combined Broadcast-and-next-Sign reauthentication must retain the exact ordinal",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
            "fn project_validated_recovered_lifecycle_signed_broadcast_and_sign_at(",
            ".filter(|record| record.owner() == next_sign_owner)\n                .count()\n                != 1",
            ".filter(|record| record.owner() == next_sign_owner)\n                .count()\n                != 0",
            "frame-bound recovered Broadcast-and-next-Sign ledger classifier omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
            "fn prepare_recovered_lifecycle_sign_broadcast_and_sign_successor<",
            "adapter.project_broadcast_and_sign_authority(body)",
            "adapter.project_broadcast_and_sign_without_body()",
            "opaque recovered Broadcast-and-next-Sign registry preparation must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs",
            "fn stage_recovered_lifecycle_sign_broadcast_and_sign_transition(",
            ".checked_add(1)",
            ".checked_add(0)",
            "inert recovered Broadcast-and-next-Sign coordinator staging must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs",
            "impl PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'_, '_, '_> {",
            "ready_index.remove(&broadcast_ordinal)",
            "ready_index.remove(&next_sign_ordinal)",
            "durable recovered Proposal Broadcast-and-next-Sign publication must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs",
            "impl PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'_, '_, '_> {",
            "adapter.commit_after_durable_broadcast_and_sign()",
            "drop(adapter)",
            "durable recovered Proposal Broadcast-and-next-Sign publication must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs",
            "impl PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'_, '_, '_> {",
            "adapter.commit_after_durable_vote_broadcast_and_sign()",
            "drop(adapter)",
            "durable recovered Broadcast-and-next-Sign publication must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "fn commit_after_durable_broadcast_and_sign(self)",
            "proposal_output_authority_minted: true",
            "proposal_output_authority_minted: _",
            "durable recovered Proposal adapter two-child commit must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "fn settle_recovered_lifecycle_proposal_broadcast_and_sign(",
            "transition.persist_exact_successor().is_err()",
            "false",
            "restart-closed recovered Proposal Broadcast-and-next-Sign settlement must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "fn settle_recovered_lifecycle_proposal_broadcast_and_sign(",
            "output.abort_before_publication()",
            "drop(output)",
            "typed recovered Proposal pre-fsync output release must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "fn settle_recovered_lifecycle_vote_broadcast_and_sign(",
            "preview.is_vote_broadcast_and_sign_shape()",
            "preview.is_vote_broadcast_and_sign()",
            "restart-closed recovered Vote Broadcast-and-next-Sign settlement must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "fn settle_recovered_lifecycle_vote_broadcast_and_sign(",
            "transition.persist_exact_successor().is_err()",
            "false",
            "restart-closed recovered Vote Broadcast-and-next-Sign settlement must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
            "fn dispatch_recovered_decision_fetch_with_runner_debt(",
            "capture_recovered_decision_fetch_exact_output(&owner)",
            "capture_recovered_decision_fetch_output_without_reservation(&owner)",
            "lifecycle-owned recovered Decision Fetch dispatch must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
            "fn persist_recovered_decision_fetch_response_after_runner(",
            "executor.prepare_recovered_decision_fetch_response_claim(&task)",
            "executor.prepare_unowned_decision_fetch_response_claim(&task)",
            "recovered Decision Fetch response persistence Phase A must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_request_registration(",
            "self.validated_certified_request_presence().is_err()",
            "false",
            "dedicated recovered Decision Fetch request owner census omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn begin_fetch<S: V2EffectServices>(",
            "owner.matches_body_coordinates(round, subject)",
            "false",
            "ordinary and recovered Decision Fetch coordinate fence omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
            "pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_body_persistence(",
            "self.revalidate_recovered_decision_fetch_response_candidate(",
            "self.trust_recovered_decision_fetch_response_candidate(",
            "typed recovered Decision Fetch selector consumption must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn commit_with_queue(",
            "owner.commit_exact_response_claim(response_hash)",
            "true",
            "recovered Decision Fetch response claim publication must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn take_io_completion(&mut self, runtime_capacity_available: bool)",
            "owned.recovered_decision_fetch.is_some()",
            "false",
            "recovered Decision Fetch mixed completion head fence must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {",
            "recovered_decision_fetch_body_completion: Option<PreparedRecoveredDecisionFetchBodyCompletionV1>,",
            "recovered_decision_fetch_body_completion: (),",
            "launched recovered Decision Fetch Drop order must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) fn settle_recovered_decision_fetch_store(",
            "transition.persist_exact_successor().is_err()",
            "false",
            "restart-closed recovered Decision Fetch-to-Store settlement must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) fn settle_recovered_decision_fetch_store(",
            "locked_dequeue.commit()",
            "drop(locked_dequeue)",
            "restart-closed recovered Decision Fetch-to-Store settlement must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
            "fn open_recovered_decision_store_startup(",
            ".authenticate_recovered_decision_fetch_store(&projection, &store_projection)",
            ".trust_recovered_decision_fetch_store(&projection, &store_projection)",
            "recovered Decision Store cold restart and marker-prefix closure omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn advance_recovered_decision_fetch_store(",
            ".project_store_adapter_authority(body)",
            ".trust_store_adapter_authority(body)",
            "recovered Decision Store cold adapter reconstruction omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery.rs",
            "pub(super) fn install_recovered_wal_decision_store(",
            "pub(super) fn install_recovered_wal_decision_store(",
            "pub(super) fn install_unchecked_recovered_wal_decision_store(",
            "dedicated recovered Decision Store registry install omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_transport.rs",
            "pub(in crate::sumeragi) fn authenticate_response(",
            "authenticate_certified_body_response_for_request(",
            "authenticate_certified_body_response_without_request(",
            "request-scoped certified response authentication omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
            "fn exactly_matches_successor_owner(",
            ".validate_authenticated_cut(&owner.serve_payloads)",
            ".validate_authenticated_cut_for_mutation(&owner.serve_payloads)",
            "CompleteTip canonical predecessor store join omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs",
            "pub(super) fn into_serve_payloads(self)",
            "pub(super) fn into_serve_payloads(self)",
            "pub(super) fn into_unsealed_payloads(self)",
            "CompleteTip bodyless completion promotion guard omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
            "pub(super) fn validate_authenticated_cut(",
            "let observed = self.reload_payload_census_strict()?;",
            "let observed = BTreeMap::new();",
            "CompleteTip body-independent Completed metadata authority omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
            "fn reload_payload_census_strict(",
            "fs::read_dir(&self.directory)",
            "fs::read_dir(temporary_path_for_mutation)",
            "CompleteTip Serve payload directory census must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
            "fn reload_payload_census_strict(",
            "fs::symlink_metadata(&self.directory)",
            "fs::metadata(&self.directory)",
            "CompleteTip Serve payload directory census must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
            "fn reload_payload_census_strict(",
            "self.load_path(&path, metadata.len())?",
            "return Ok(payloads);",
            "CompleteTip Serve payload directory census must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
            "pub(crate) struct ProductionLifecycleOwnerV1",
            "serve_payloads: crate::sumeragi::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut,",
            "serve_payloads: (),",
            "production lifecycle owner retained Serve census omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
            "fn run_complete_tip_retirement_release_regressions()",
            "ledger::tests::durable_ready_fetch_recovery::complete_tip_retirement_binds_only_the_exact_unlaunched_successor_owner();",
            "let _ = ();",
            "production lifecycle owner retained Serve census omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_first_release_recovery.rs",
            "pub(crate) use super::v2_lifecycle_coordinator::{",
            "run_complete_tip_retirement_release_regressions",
            "run_unchecked_complete_tip_retirement_release_regressions",
            "CompleteTip first-release recovery seam omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
            "fn authorizes_successor_status(",
            "self.complete_tip.successor_context_id() == successor.height_context_id",
            "true",
            "CompleteTip restart publication authority must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
            "pub(in crate::sumeragi) struct BoundRecoveredCompleteTipSuccessorOwnerV1 {",
            "#[cfg(test)]\nimpl BoundRecoveredCompleteTipSuccessorOwnerV1 {",
            "impl BoundRecoveredCompleteTipSuccessorOwnerV1 {\n"
            "    pub(in crate::sumeragi) fn into_owner(self) -> ProductionLifecycleOwnerV1 { self.owner }\n"
            "}\n\n"
            "#[cfg(test)]\nimpl BoundRecoveredCompleteTipSuccessorOwnerV1 {",
            "CompleteTip exact H+1 owner bind must use the opaque checked-transition gate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "pub(in crate::sumeragi) fn authorizes(\n        self,",
            "self.kura_identity.matches(kura)",
            "true",
            "recovered lifecycle storage authority handoff omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn mint_from_recovered_height(",
            "assert!(permit.authorizes(kura, verified, signature_policy, genesis_account));",
            "assert!(true);",
            "recovery-minted lifecycle storage authority omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) struct ProductionLifecycleLaunchInputsV1 {",
            "authenticated_genesis: Option<AuthenticatedGenesisBodyV1>,",
            "authenticated_genesis: Option<AuthenticatedGenesisBodyV1>,\n    genesis_account: AccountId,",
            "move-only authenticated genesis launch input must use the opaque checked-transition gate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) fn launch(\n        mut self,",
            "binding.matches_launch_identity(inputs.kura.as_ref(), &inputs.key_pair)",
            "true",
            "Kura-bound production lifecycle launch must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "fn launch_local_identity_matches(",
            "local_peer.public_key() != key_pair.public_key()",
            "false",
            "local launch identity preflight omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "fn launch_local_identity_matches(",
            "local_validator.is_none_or(|observed| roster_position == Some(observed))",
            "local_validator.is_none_or(|_| true)",
            "local launch identity preflight omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn mint_for_recovered_runner(local_signer: KeyPair) -> Self",
            "fn mint_for_recovered_runner(local_signer: KeyPair) -> Self",
            "pub(in crate::sumeragi) fn mint_for_recovered_runner(local_signer: KeyPair) -> Self",
            "runner-sealed recovered lifecycle factory dependencies must use the opaque checked-transition gate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn bind_production_lifecycle_owner_factory_inputs_v1(",
            "let local_signer = permit.into_local_signer();",
            "let local_signer = KeyPair::random();",
            "recovery-minted lifecycle storage authority omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(",
            "self.adapter.wal.matches_path(&storage.wal_path)",
            "true",
            "canonical Kura-bound lifecycle-owner factory must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(",
            "Arc::ptr_eq(&adapter_owner, &self.factory_owner)",
            "true",
            "canonical Kura-bound lifecycle-owner factory must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(",
            "body_store: super::v2_body_store::QuarantinedV2BodyStore",
            "body_store: super::v2_body_store::V2BodyStore",
            "canonical Kura-bound lifecycle-owner factory must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(",
            ".into_revalidated_lifecycle_startup(",
            ".into_revalidated_startup(",
            "canonical Kura-bound lifecycle-owner factory must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_body_store.rs",
            "pub(in crate::sumeragi) fn into_quarantined_recovered_startup(",
            "!self.validated.is_empty()",
            "false",
            "fresh quarantined recovered body-store cut omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_body_store.rs",
            "pub(in crate::sumeragi) fn into_revalidated_lifecycle_startup(",
            "apply_service.recovered_finality_subject(context)",
            "None::<VerifiedRecoveredFinalitySubject>.ok_or(())?",
            "fixed quarantined recovered marker replay must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_body_store.rs",
            "pub(in crate::sumeragi) fn into_revalidated_lifecycle_startup(",
            ".retain_recovered_markers_for_authority(validation_authority)",
            ".retain_recovered_markers_for_mutation(validation_authority)",
            "fixed quarantined recovered marker replay must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_body_store.rs",
            "pub(in crate::sumeragi) fn into_revalidated_lifecycle_startup(",
            ".revalidate_recovered_markers(|body|",
            ".retain_recovered_markers_for_mutation(|body|",
            "fixed quarantined recovered marker replay must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn bind_production_lifecycle_owner_factory_inputs_v1(",
            "state.matches_kura_instance(&kura)",
            "true",
            "recovery-minted lifecycle storage authority omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) fn launch(\n        mut self,",
            "ProductionV2Services::start_with_apply_service(",
            "ProductionV2Services::start(",
            "Kura-bound production lifecycle launch must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) fn launch(\n        mut self,",
            "ProductionLifecycleApplyServiceLaunchPermitV1 {",
            "ForgedProductionLifecycleApplyServiceLaunchPermitV1 {",
            "sealed replay-service permit mint must contain",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
            "pub(in crate::sumeragi) fn with_recovered_kura_binding_and_apply_service(",
            "self.apply_service = Some(apply_service);",
            "drop(apply_service);",
            "production lifecycle owner Kura seal omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(in crate::sumeragi) fn start_with_apply_service(",
            "apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops)",
            "true",
            "sealed replay-service worker transfer omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/state.rs",
            "pub(crate) fn matches_kura_instance(",
            "Arc::ptr_eq(&self.kura, kura)",
            "true",
            "fixed State/Kura identity oracle omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "pub(in crate::sumeragi) fn matches_lifecycle_launch(",
            "Arc::ptr_eq(&self.state, state)",
            "true",
            "fixed recovered Apply-service identity oracle omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn prepare_leader_wire_launch(",
            "adapter.wal.matches_path(expected_wal_path)",
            "true",
            "sealed adapter leader-wire launch projection omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/safety_wal.rs",
            "fn publish_atomic(&self, frame: &[u8], maximum: u64, label: &str)",
            "let durable = rustix::fs::statat(",
            "let durable = promoted;",
            "opened safety-WAL directory authority omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
            "pub(crate) fn open_with_safety_wal_authority(\n"
            "        storage: SafetyWalServicedCandidateStoreAuthority,",
            "storage: SafetyWalServicedCandidateStoreAuthority",
            "storage: SafetyWalLeaderWireStoreAuthority",
            "typed WAL-adjacent production stores omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn prepare_leader_wire_launch(",
            "*leader_wire_launch_prepared = true;",
            "let _ = leader_wire_launch_prepared;",
            "sealed adapter leader-wire launch projection omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(in crate::sumeragi) fn open_gate(",
            "body_store\n            .recovery_catalog()",
            "BTreeMap::new()",
            "sealed adapter leader-wire launch projection omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) fn launch(\n        mut self,",
            "leader_wire_launch.restored_producer_ordinal_high_watermark()",
            "None",
            "Kura-bound production lifecycle launch must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) fn launch(\n        mut self,",
            "leader_wire_restore.scheduler_ordinal_high_watermark()",
            "0",
            "Kura-bound production lifecycle launch must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "fn retire(&mut self) -> Result<(), String>",
            "self.ingress.unbind_leader_wire_lifecycle_gate(gate)?",
            "self.gate = None;",
            "sealed leader-wire launch binding omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_context.rs",
            "pub fn freeze_staged_genesis_v2(",
            "let authenticated_genesis = AuthenticatedGenesisBodyV1::authenticate(genesis)?;",
            "let authenticated_genesis = forged_authenticated_genesis;",
            "signed genesis bootstrap seal mint omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_context.rs",
            "pub struct GenesisV2Bootstrap {",
            "pub struct GenesisV2Bootstrap {",
            "#[derive(Debug, Clone)]\npub struct GenesisV2Bootstrap {",
            "move-only authenticated genesis bootstrap must use the opaque checked-transition gate",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "pub struct GenesisWithPubKey {",
            "pub struct GenesisWithPubKey {",
            "#[derive(Debug, Clone)]\npub struct GenesisWithPubKey {",
            "move-only genesis runner bundle must use the opaque checked-transition gate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "pub(crate) fn recover_active_height_with_plan(",
            "if !authenticated_genesis.authorizes(&genesis_public_key) {",
            "if false {",
            "recovery-sealed fresh genesis handoff omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "pub(crate) fn recover_active_height_with_plan(",
            "authenticated_genesis: Some(authenticated_genesis),",
            "authenticated_genesis: None,",
            "recovery-sealed fresh genesis handoff omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            "pub(in crate::sumeragi) fn launch(\n        mut self,",
            "authenticated_genesis.signed_block()",
            "forged_genesis_body_for_mutation",
            "move-only authenticated genesis launch input omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
            "fn authorizes_retained_successor(",
            "self.predecessor_store.load().ok().as_ref() == Some(&self.predecessor_ledger)",
            "true",
            "CompleteTip restart publication authority must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/height_ingress_bindings.rs",
            "fn open_ingress_for_active_height(",
            "output_guard.begin_fail_stop_operation()",
            "output_guard.acquire()",
            "open_ingress_for_active_height must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn run_inner(",
            "SumeragiV2Adapter::open_deferred_status_with_capacity_geometry(",
            "SumeragiV2Adapter::open_deferred_status(",
            "run_inner live successor startup must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn run_inner(",
            "V2EffectExecutor::open_with_body_store(",
            "V2EffectExecutor::open(",
            "run_inner live successor startup must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            ".serve_historical_body(kura, request, &sender, local_key)",
            ".serve_historical_body(kura, context_store, request, &sender, local_key)",
            "historical ingress routing omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "executor.accept_certified_body_response_with_ingress_ownership(\n"
            "                    response,\n"
            "                    &sender,\n"
            "                    &ingress_ownership,\n"
            "                    services,\n"
            "                )",
            "executor.accept_certified_body_response_with_ingress_ownership(\n"
            "                    response,\n"
            "                    &sender,\n"
            "                    ingress_ownership,\n"
            "                    services,\n"
            "                )",
            "historical ingress routing omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/height_ingress_bindings.rs",
            "fn initial_block_sync_deadline(",
            "if eager_recovery {\n        height_started_at\n    } else {",
            "if eager_recovery {\n"
            "        deadline_after(height_started_at, round_timeout)\n"
            "    } else {",
            "recovery-scoped eager block-sync initial_block_sync_deadline "
            "declaration and complete control flow",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn run_inner(",
            "admitted_discovered_commit_qc = true;",
            "admitted_discovered_commit_qc = false;",
            "only authenticated discovered CommitQC admission/coalescing with "
            "serialized reducer ownership may turn an outstanding request from "
            "Some to None and retain eager block-sync",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn run_inner(",
            "let (receipt, artifact, lane_work, mut finalized_services) = finality;",
            "let (receipt, artifact, _lane_work, mut finalized_services) = finality;",
            "successor startup must carry interrupted-tip or admitted discovered "
            "CommitQC recovery and clear ordinary live finality",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/height_ingress_bindings.rs",
            "const fn retain_eager_block_sync(",
            "recovering_interrupted_tip || admitted_discovered_commit_qc",
            "{ let _ = admitted_discovered_commit_qc; recovering_interrupted_tip }",
            "recovery-scoped eager block-sync retain_eager_block_sync "
            "declaration and complete control flow",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn publish_recovered_v2_successor_height_at(",
            "set_v2_status_at(successor, now);",
            "update_v2_successor_work_stage_at(finalized_height, SumeragiV2LocalWorkStage::Running, SumeragiV2LocalWorkStage::Complete, now)?; set_v2_status_at(successor, now);",
            "may not fabricate physical predecessor completion",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn activate_v2_successor_height_at(",
            "validate_v2_predecessor_status(\n"
            "        &predecessor_status,\n"
            "        finalized_height,\n"
            "        SumeragiV2LocalWorkStage::Running,\n"
            "    )?;",
            "let _ = &predecessor_status;",
            "activate_v2_successor_height_at omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn activate_v2_successor_height_at(",
            "predecessor_status_height: predecessor_status.height,",
            "predecessor_status_height: finalized_height,",
            "activate_v2_successor_height_at omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn activate_v2_successor_height_at(",
            "update_v2_successor_work_stage_at(\n"
            "        finalized_height,\n"
            "        SumeragiV2LocalWorkStage::Running,\n"
            "        SumeragiV2LocalWorkStage::Complete,\n"
            "        now,\n"
            "    )?;",
            "update_v2_successor_work_stage_at(finalized_height, SumeragiV2LocalWorkStage::Running, SumeragiV2LocalWorkStage::Running, now)?;",
            "activate_v2_successor_height_at omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn activate_v2_successor_height_at(",
            "let _authorized_trace = checked_trace.into_projection();",
            "drop(checked_trace);",
            "activate_v2_successor_height_at omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "pub(crate) fn begin_v2_successor_activation(",
            "let _authorized_lifecycle = checked_lifecycle.into_projection();\n"
            "    update_v2_successor_work_stage_at(\n"
            "        height,\n"
            "        SumeragiV2LocalWorkStage::Queued,\n"
            "        SumeragiV2LocalWorkStage::Running,\n"
            "        Instant::now(),\n"
            "    )",
            "let mutation_result = update_v2_successor_work_stage_at(\n"
            "        height,\n"
            "        SumeragiV2LocalWorkStage::Queued,\n"
            "        SumeragiV2LocalWorkStage::Running,\n"
            "        Instant::now(),\n"
            "    );\n"
            "    let _authorized_lifecycle = checked_lifecycle.into_projection();\n"
            "    mutation_result",
            "begin_v2_successor_activation must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn publish_recovered_v2_successor_height_at(",
            "published_status_height_before: published.as_ref().map_or(0, |status| status.height),",
            "published_status_height_before: 0,",
            "publish_recovered_v2_successor_height_at omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn publish_recovered_v2_successor_height_at(",
            "if let Some(published) = published {\n"
            "        return Err(V2SuccessorActivationError::RecoveredStatusAlreadyPublished(\n"
            "            published.height,\n"
            "        ));\n"
            "    }\n"
            "    set_v2_status_at(successor, now);",
            "set_v2_status_at(successor, now);",
            "publish_recovered_v2_successor_height_at must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "pub(crate) fn begin_v2_successor_activation(",
            "stage_before: successor_stage_projection(status.liveness.work.successor_height),",
            "stage_before: SUCCESSOR_STAGE_QUEUED,",
            "begin_v2_successor_activation omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "pub(crate) fn mark_v2_restart_required()",
            '"Sumeragi v2 Running successor failure projection was rejected; preserving the unchecked status"\n'
            "                );\n"
            "                return;",
            '"Sumeragi v2 Running successor failure projection was rejected; preserving the unchecked status"\n'
            "                );",
            "mark_v2_restart_required must contain 'return;' exactly 2 time(s)",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "pub(crate) fn mark_v2_restart_required()",
            "check_production_successor_startup_lifecycle_transition(lifecycle)",
            "Some(lifecycle)",
            "mark_v2_restart_required omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn recovered(\n        authority: RecoveredSuccessorActivationAuthority,",
            "let published_height = super::status::v2_status().map_or(0, |status| status.height);",
            "let published_height = 0;",
            "PendingSuccessorActivation omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn recovered(\n        authority: RecoveredSuccessorActivationAuthority,",
            "let Some(checked_lifecycle) =\n"
            "            check_production_successor_startup_lifecycle_transition(lifecycle)\n"
            "        else {\n"
            "            return Err(V2RunnerError::SuccessorRefinementRejected);\n"
            "        };\n"
            "        let _authorized_lifecycle = checked_lifecycle.into_projection();",
            "if !production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(\n"
            "            lifecycle,\n"
            "        ) {\n"
            "            return Err(V2RunnerError::SuccessorRefinementRejected);\n"
            "        }",
            "must use the opaque checked-transition gate; found obsolete direct-kernel forms",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn bind(\n        self,",
            "authority_predecessor: authority.predecessor().refinement_projection(),",
            "authority_predecessor: self.predecessor.refinement_projection(),",
            "PendingSuccessorConstruction omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "pub(crate) fn authenticate(\n        artifact: &wire::finality::V2FinalityArtifact,",
            "|| receipt.certificate() != artifact.commit_qc.as_ref()",
            "|| false",
            "DurableV2PredecessorIdentity::authenticate omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "pub(crate) fn authenticate(\n        artifact: &wire::finality::V2FinalityArtifact,",
            "if !production_durable_predecessor_identity_kernel(identity.refinement_projection()) {",
            "if false {",
            "DurableV2PredecessorIdentity::authenticate omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "fn new(record: &wire::SnapshotV2BootstrapRecord) -> Self",
            "record_hash: HashOf::new(record),",
            "record_hash: HashOf::new(&wire::SnapshotV2BootstrapRecord::default()),",
            "SnapshotSuccessorActivationAuthority::new omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "pub(crate) fn recover_active_height_with_plan(",
            "if record.context() != &bootstrap.context\n"
            "            || record.proofs_of_possession() != bootstrap.validator_set_pops",
            "if false",
            "recover_active_height_with_plan snapshot authority omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "pub(crate) fn recover_active_height_with_plan(",
            "v2_finality_artifact_with_receipt(durable_height)",
            "v2_finality_artifact(durable_height)",
            "recover_active_height_with_plan complete-tip authority omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "    fn register_parent_qc(",
            "if !reference.same_commit_decision(frozen) {",
            "if false {",
            "WireRegistry::register_parent_qc omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "    fn justification_to_core(",
            ".map(|certificate| self.register_parent_qc(certificate))",
            ".map(|certificate| self.qc_reference_to_core(&certificate.as_ref()))",
            "WireRegistry::justification_to_core omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "fn verify_proposal_justification_authority(",
            "(Some(certificate), Some(parent_verification)) => verify_quorum_certificate(",
            "(Some(_), Some(_)) => Ok(",
            "verify_proposal_justification_authority omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
            "fn build_historical_body_response(",
            ".position(|entry| entry.validator == responder_peer)",
            ".any(|entry| entry.validator == responder_peer)",
            "build_historical_body_response must preserve exact production order",
        ),
        (
            "scripts/run_sumeragi_v2_release_gates.sh",
            "required_production_liveness_tests=(",
            "sumeragi::v2_block_sync::tests::catch_up_is_strictly_sequential_across_contexts",
            "sumeragi::v2_block_sync::tests::catch_up_is_not_release_bound",
            "production refinement test must be pinned exactly once",
        ),
    ),
)
def test_successor_production_source_mapping_mutations_fail_closed(
    tmp_path: Path,
    relative_path: str,
    region_marker: str,
    old: str,
    new: str,
    error_fragment: str,
) -> None:
    module = load_checker()
    for source_name in (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "crates/iroha_core/src/sumeragi/mod.rs",
        "crates/iroha_core/src/sumeragi/status.rs",
        "crates/iroha_core/src/sumeragi/v2_first_release_recovery.rs",
        "crates/iroha_core/src/sumeragi/v2.rs",
        "crates/iroha_core/src/sumeragi/v2_runtime.rs",
        "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "crates/iroha_core/src/sumeragi/v2_context.rs", "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "crates/iroha_core/src/sumeragi/v2_body_store.rs",
        "crates/iroha_core/src/sumeragi/safety_wal.rs",
        "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "crates/iroha_core/src/sumeragi/v2_transport.rs",
        "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "crates/iroha_core/src/state.rs",
        "crates/iroha_core/src/kura.rs",
        "scripts/run_sumeragi_v2_release_gates.sh",
    ):
        destination = tmp_path / source_name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / source_name, destination)
    copy_reviewed_rust_include_components(tmp_path)

    assert module._successor_production_source_fidelity_errors(tmp_path) == []

    path = tmp_path / relative_path
    source = path.read_text(encoding="utf-8")
    region_start = source.find(region_marker)
    assert region_start >= 0
    mutation = source.find(old, region_start)
    assert mutation >= 0
    function_name = re.search(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)", region_marker)
    if function_name is not None:
        owning_items = []
        for item in module.rust_items(source, function_name.group(1)):
            item_start = source.find(item.source)
            if item_start <= region_start < item_start + len(item.source):
                owning_items.append((item_start, item))
        assert len(owning_items) == 1, (
            "region marker did not select exactly one production Rust item",
            relative_path,
            region_marker,
        )
        item_start, item = owning_items[0]
        assert item_start <= mutation < item_start + len(item.source), (
            "mutation escaped the production Rust item selected by its region marker",
            relative_path,
            region_marker,
            old,
        )
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    errors = module._successor_production_source_fidelity_errors(tmp_path)
    assert any(error_fragment in error for error in errors), errors
    if (
        relative_path == "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs"
        and region_marker == "fn authorizes_retained_successor("
    ):
        successor_reload = (
            "self.successor_store.load().ok().as_ref() "
            "== Some(&self.successor_ledger)"
        )
        assert source.count(successor_reload) == 1
        path.write_text(source.replace(successor_reload, "true", 1), encoding="utf-8")
        errors = module._successor_production_source_fidelity_errors(tmp_path)
        assert any(error_fragment in error for error in errors), errors
