"""Exact current native test ownership for the Taira development gate.

This static census binds test declarations to registered Rust source owners.
It is not release evidence; executable --list verification and actual execution remain required.
Signed RS16 payload transport qualification remains open (specs/sumeragi_goals.md, question 8).
"""
from pathlib import Path
import re

# (coverage, parent source, test source, registered module, full module path, exact test leaves)
NATIVE_CORE_TEST_OWNERS = (
    ('native publication custody', 'sumeragi/executor.rs', 'sumeragi/executor_publication_tests.rs', 'publication_tests', 'sumeragi::executor::publication_tests', (
        'reversible_publication_refusal_retains_original_overlay_capture_and_certified_frame',
        'prepared_certificate_cannot_be_rebound_to_another_epoch_context',
        'preparation_pins_original_even_against_discard_replacement_and_another_valid_quorum',
        'mismatching_certified_result_reports_original_without_reexecuting_or_pinning_it',
        'consuming_publication_failure_blocks_every_reexecution_path',
        'panic_after_actual_visibility_is_local_recovery_never_success_or_reexecution',
        'original_worker_consuming_failure_halts_driver_status_without_reexecution',
        'state_busy_retry_retains_original_metadata_snapshot_certificate_and_pool_custody',
        'context_proof_capacity_retry_retains_original_witness_inputs_and_execution',
        'result_encoding_capacity_retry_keeps_original_execution_and_allocation_custody',
        'result_encoding_foreign_pool_requires_recovery_and_retains_original_execution',
        'qc_encoding_refusal_retains_original_header_result_and_execution_until_capacity_returns',
        'certificate_control_refusal_retains_every_original_part_through_publication_retry',
        'native_control_is_attached_once_and_remote_refusal_preserves_the_original_owner',
        'native_control_never_displaces_the_original_prepared_publication',
        'quarantine_requires_the_exact_control_free_transaction_rejection_hash',
        'native_header_source_is_checked_against_the_pristine_committed_parent',
        'state_executor_serializes_real_control_requests_and_one_time_attachment',
        'native_attestation_attachment_retains_the_first_original_pool_mailbox',
        'native_prepare_checks_real_quorum_before_changing_original_publication_owners',
        'native_pasta_refusals_retain_original_execution_until_actual_receipt_publication',
        'native_pasta_discard_invalidates_the_receipt_before_releasing_its_original_execution',
        'native_context_archive_capacity_retry_retains_original_overlay_and_result',
        'native_context_archive_failure_preserves_original_bytes_until_durable_acknowledgement',
        'native_context_archive_preparation_refuses_foreign_pool_without_reexecuting',
        'availability_encoding_refusal_retains_original_header_qc_and_execution',
        'canonical_replay_origin_retains_transition_idempotence_through_publication_retry',
        'malformed_available_payload_remains_invalid_and_negatively_cached',
        'payload_decode_refusal_retains_available_owner_without_negative_cache',
    )),
    ('native durable archive recovery', 'sumeragi/executor.rs', 'sumeragi/executor/archive_tests.rs', 'archive_tests', 'sumeragi::executor::archive_tests', (
        'partial_archive_failure_retains_exact_decision_and_retries_without_reexecution_or_notifications',
        'pending_capture_rejects_substituted_header_qc_state_and_missing_certificate',
        'archive_attachment_captures_exact_tip_once_before_executor_work_and_survives_reopen',
        'below_quorum_current_frame_cannot_finish_pending_archive_capture',
        'restart_binds_archives_after_replay_and_captures_the_missing_tip_once',
        'binding_refuses_unpublished_execution_and_captures_the_published_tip',
    )),
    ('native original execution and undo', 'state/native_execution_tip.rs', 'state/native_execution_tip/tests.rs', 'tests', 'state::native_execution_tip::tests', (
        'original_genesis_and_worker_publish_exact_tip_with_undo',
        'rejected_original_preparation_and_discard_preserve_committed_tip',
        'replacement_reads_exact_tip_undo_and_abandonment_restores_original_cut',
        'execution_history_admits_each_actual_source_before_read',
        'restore_reauthenticates_native_current_and_undo_and_rejects_claim_substitution',
        'genesis_only_snapshot_cannot_decode_its_unsigned_result_into_authority',
        'funded_tip_admission_is_atomic_and_original_pool_bound',
        'snapshot_json_preserves_genesis_undo_absence_distinction',
    )),
    ('native certified history', 'sumeragi/certified_chain.rs', 'sumeragi/certified_chain/tests.rs', 'tests', 'sumeragi::certified_chain::tests', (
        'committed_and_certified_reads_of_a_real_chain',
        'uncommitted_heights_are_not_read',
        'frames_without_a_matching_header_preimage_or_certificate_are_refused',
        'certificates_that_do_not_certify_the_stored_block_are_refused',
        'two_valid_certificates_of_one_block_give_one_consensus_receipt',
        'a_view_of_another_network_is_refused',
        'historical_committee_material_cannot_bypass_global_voting_geometry',
        'genesis_signature_is_verified_even_when_its_header_hash_matches_the_view',
        'genesis_payload_is_bound_to_its_signed_header_before_authority_is_read',
        'installing_an_attestation_verifier_rechecks_the_previously_verified_prefix',
        'pinned_prefix_uses_the_exact_cut_without_a_world_authority',
        'pinned_prefix_rejects_empty_foreign_changed_and_unavailable_sources',
        'pinned_genesis_result_is_unsigned_until_a_real_successor_authenticates_it',
        'portable_committee_uses_the_authenticated_original_epoch_members',
        'borrowed_native_frames_use_the_same_verifier_and_exact_cut',
        'borrowed_native_frames_reject_changed_result_even_under_unchanged_header_hash',
        'certified_reader_rejects_missing_foreign_and_corrupt_signed_availability',
        'durable_certificate_read_rejects_checksum_valid_corruption_after_cache_warm',
    )),
    ('native certified history boundaries', 'sumeragi/certified_chain/tests.rs', 'sumeragi/certified_chain/boundary_tests.rs', 'boundaries', 'sumeragi::certified_chain::tests::boundaries', (
        'rotated_away_committee_verifies_from_authenticated_boundaries_with_bounded_authority',
        'retained_generation_still_binds_new_epoch_and_fresh_leader_randomness',
        'historical_authority_missing_reordered_or_forged_proofs_fail_closed',
        'boundary_authority_and_parent_links_cannot_self_authorize',
        'nonempty_boundary_requires_flagged_attestation_and_exact_fresh_pulse',
        'unsigned_genesis_result_cannot_substitute_the_signed_epoch_root',
        'result_pulses_require_exact_height_network_session_and_parent_bindings',
        'pinned_restoration_reuses_full_boundary_verification_and_one_authority_cursor',
        'historical_pulse_requires_signed_header_bytes_and_complete_native_context',
    )),
    ('native certified prefix authority', 'sumeragi/certified_chain/tests.rs', 'sumeragi/certified_chain/prefix_tests.rs', 'prefix_tests', 'sumeragi::certified_chain::tests::prefix_tests', (
        'streamed_prefix_emits_genesis_execution_anchor_only_after_real_successor',
        'unsigned_changed_genesis_result_cannot_be_exported_by_streamed_reader',
        'streamed_prefix_checks_genuine_pasta_at_retained_empty_epoch_boundary',
    )),
    ('native pending original execution', 'sumeragi/test_chain.rs', 'sumeragi/test_chain/pending_execution_tests.rs', 'pending_execution_tests', 'sumeragi::test_chain::tests::pending_execution_tests', (
        'pending_execution_retains_one_source_and_publishes_once',
        'original_wire_witness_and_world_tampering_fail_before_durable_staging',
        'pending_execution_rejects_inexact_quorum_without_publication',
        'discarded_unprepared_execution_leaves_original_state_unchanged',
        'prepared_genesis_derives_then_enforces_both_signed_native_policies',
        'changed_original_genesis_configuration_is_rejected_before_publication',
    )),
    ('native original publication', 'sumeragi/test_chain.rs', 'sumeragi/test_chain/native_publication_tests.rs', 'native_publication_tests', 'sumeragi::test_chain::tests::native_publication_tests', (
        'original_genesis_and_successor_have_exact_native_execution_authority',
        'foreign_execution_certificate_cannot_prepare_original_worker',
        'publication_requires_the_original_nonempty_captured_witness',
        'original_signed_genesis_refuses_foreign_validator_custody',
        'missing_genesis_authority_is_created_by_its_original_signed_registration',
    )),
    ('native witness admission', 'sumeragi/driver/mod.rs', 'sumeragi/driver/tests/witness_admission.rs', 'witness_admission_tests', 'sumeragi::driver::witness_admission_tests', (
        'pending_remote_frame_preserves_original_backing_and_never_enters_core_early',
        'pending_slot_rejects_foreign_owners_and_retained_handle_cannot_revive_stopped_instance',
        'closed_instance_releases_original_witnesses_even_with_a_retained_handle',
    )),
    ('native lane merge authority', 'sumeragi/lanes/merge.rs', 'sumeragi/lanes/merge_tests.rs', 'tests', 'sumeragi::lanes::merge::tests', (
        'global_blocks_merge_fresh_lane_blocks_and_drop_what_they_must_not_execute',
        'malformed_merges_are_invalid_and_missing_blocks_pending',
        'expansion_consumes_only_the_exact_original_proposal',
        'expansion_refuses_equivalent_foreign_state_and_changed_publication',
        'merged_rejection_event_retains_the_original_native_proposal_header',
        'leader_proposal_preserves_local_storage_error_instead_of_omitting_lane_work',
    )),
    ('native lane signer custody', 'sumeragi/lanes/custody.rs', 'sumeragi/lanes/custody/tests.rs', 'tests', 'sumeragi::lanes::custody::tests', (
        'lane_obligation_never_moves_to_a_later_registration_sharing_key_and_account',
        'every_original_incarnation_must_finish_its_delay_before_custody_releases',
        'malformed_original_deadline_cannot_release_retained_custody',
        'original_signer_binding_requires_positive_custody_and_survives_policy_member_order',
        'retirement_marks_exact_boundary_and_policy_extension_precedes_withdrawal',
        'creation_pins_once_and_capacity_is_reclaimed_only_after_retirement_delay',
        'pending_original_evidence_delays_reclamation_without_native_height_arithmetic',
        'retirement_keeps_the_final_same_carrier_merge_after_the_live_record_is_removed',
        'an_existing_frontier_cannot_regress_or_switch_hash_or_result_at_the_same_native_height',
        'original_signer_pinning_refuses_then_retries_the_same_pool_and_stake_cut',
        'original_signer_state_handoff_retains_backing_and_refuses_foreign_pool',
        'original_signer_world_handoff_admits_both_generations_before_replacing_either',
        'original_signer_state_constructor_refuses_before_cloning_unfunded_world',
        'sample_state_admission_refuses_unfunded_source',
        'original_sample_state_constructor_refuses_with_typed_sample_cause',
        'original_sample_world_handoff_admits_both_generations_before_replacing_either',
    )),
    ('native lane signer restore custody', 'state/deserialize_world.rs', 'state/deserialize_world_lane_custody_tests.rs', 'native_lane_custody_tests', 'state::deserialize::native_lane_custody_tests', (
        'native_lane_signer_snapshot_retains_exact_raw_source_until_both_cuts_are_funded',
        'native_lane_signer_snapshot_rejects_noncanonical_fields_without_consuming_source',
        'native_lane_signer_snapshot_decode_refusal_retains_category_and_original_field',
        'native_lane_sample_snapshot_retains_raw_source_through_both_cut_refusal_and_retry',
    )),
    ('native lane signer restore errors', 'snapshot/errors.rs', 'snapshot/errors/native_lane_custody_tests.rs', 'native_lane_custody_tests', 'snapshot::errors::native_lane_custody_tests', (
        'restore_lane_custody_refusal_keeps_original_typed_local_error',
    )),
    ('native beacon custody', 'sumeragi/epoch_beacon/producer.rs', 'sumeragi/epoch_beacon/producer/tests.rs', 'tests', 'sumeragi::epoch_beacon::producer::tests', (
        'all_seats_drive_real_shares_once_and_followers_use_only_transported_pulse',
        'wrong_source_sender_and_proof_never_change_the_owned_round',
        'explicitly_anchored_no_demand_needs_neither_session_nor_fake_observer_key',
        'refusal_classification_keeps_remote_faults_out_of_local_recovery',
        'native_invalid_local_share_is_never_retained_or_counted',
        'native_transient_signer_refusal_retains_remote_progress_and_exact_retry_payload',
        'native_mandatory_slot_starts_without_transaction_work_and_missing_key_refuses',
        'native_active_session_from_a_foreign_real_committee_refuses_before_signing',
        'readiness_reprobes_same_applied_cut_without_signing_and_excludes_stale_generation',
        'readiness_no_demand_does_not_require_a_beacon_session',
        'control_retries_use_original_tip_without_decoding_history_again',
        'control_requires_original_tip_and_matching_published_hash_journal',
    )),
    ('native executed beacon controls', 'sumeragi/epoch_beacon/producer/tests.rs', 'sumeragi/epoch_beacon/producer/execution_tests.rs', 'execution_tests', 'sumeragi::epoch_beacon::producer::tests::execution_tests', (
        'transported_pulse_executes_once_and_cold_replay_reproduces_the_certified_result',
        'native_pulse_refusals_preserve_the_exact_predecessor_and_require_actual_work',
    )),
    ('native frozen epoch schedule', 'sumeragi/schedule.rs', 'sumeragi/schedule/tests.rs', 'tests', 'sumeragi::schedule::tests', (
        'chain_params_record_round_trips_the_core_and_on_chain_forms',
        'chain_params_validation_uses_the_chain_transport_limit',
        'consensus_key_maps_bls_peers_both_ways_and_rejects_others',
        'canonical_committee_sorts_by_core_key_and_dedups',
        'parameter_changes_validate_after_genesis_only',
        'genesis_committee_reads_the_signed_validator_registrations',
        'signed_genesis_requires_exact_global_geometry_without_observer_padding',
        'genesis_committee_rejects_bad_registrations',
        'global_committees_have_exact_bounded_geometry_and_equal_vote_quorums',
        'native_epoch_binds_core_authority_and_rejects_missing_reordered_or_changed_original_proofs',
        'boundary_application_is_the_only_cut_that_replaces_pending_authority',
        'scheduled_slot_json_requires_its_canonical_variant_tag',
        'restored_schedule_keeps_original_generation_and_full_epoch_graph',
        'genesis_outcome_cannot_supply_an_arbitrary_epoch_or_boundary',
    )),
    ('native executed genesis registration', 'sumeragi/schedule/execution.rs', 'sumeragi/schedule/execution_tests.rs', 'tests', 'sumeragi::schedule::execution::tests', (
        'executed_genesis_must_retain_every_exact_signed_registration',
        'executed_genesis_rejects_an_extra_voting_registration',
    )),
    ('native driver persistence', 'sumeragi/driver/tests/mod.rs', 'sumeragi/driver/tests/file_stores.rs', 'file_stores', 'sumeragi::driver::tests::file_stores', (
        'file_stores_survive_disk_errors',
    )),
    ('native driver scheduling', 'sumeragi/driver/tests/mod.rs', 'sumeragi/driver/tests/sched.rs', 'sched', 'sumeragi::driver::tests::sched', (
        'most_recent_first_and_parking',
        'discard_cancels_waiting_and_running',
        'commit_during_execution_executes_once',
        'commit_answers_a_queued_execution',
        'missing_cache_recomputes_and_mismatch_diverges',
        'apply_failures_are_retried',
        'build_after_parent_apply_and_payload_ready',
        'arrival_during_a_build_follows_an_empty_answer',
        'applied_heights_answer_waiting_requests',
        'every_execute_is_answered_exactly_once',
        'executor_panics_become_local_failures',
        'apply_runs_alone_while_backing_off',
        'failed_commit_prepares_again_without_a_second_append',
        'repeated_commit_failures_preserve_backoff_and_reset_after_success',
        'commit_backoff_survives_a_failed_reprepare',
        'discards_merge_and_rejections_are_bounded',
        'terminal_publication_errors_stop_all_scheduled_work',
        'cached_result_mismatch_diverges_without_reexecution',
        'original_boundary_config_survives_retry_and_is_delivered_atomically',
        'control_build_refusal_keeps_exact_request_and_does_not_block_transaction_work',
        'control_build_view_change_cancels_queued_and_running_retry',
        'all_validator_control_waits_for_applied_parent_and_shares_do_not_starve_work',
        'application_control_ingress_has_a_hard_protocol_cap',
        'control_worker_unwind_requires_recovery_and_cannot_invent_empty',
        'due_control_build_progresses_under_replenished_drive_and_partial_ingress',
    )),
    ('native driver kernel', 'sumeragi/driver/tests/mod.rs', 'sumeragi/driver/tests/kernel.rs', 'kernel', 'sumeragi::driver::tests::kernel', (
        'tick_first_then_local_then_messages',
        'own_and_foreign_messages_are_dropped',
        'barrier_and_ordered_persistence',
        'served_bodies_become_local_events',
        'serving_is_bounded_and_the_nodes_fetch_goes_first',
        'failing_disk_bounds_the_queues_and_releases_in_batches',
        'publication_recovery_halts_before_poll_and_preserves_safety_persistence',
        'frame_limits_cover_both_atomic_boundary_configs_without_pending_fallback',
        'core_discard_routes_the_same_authorized_keep_set_to_payload_lifetime',
    )),
    ('native driver fault conformance', 'sumeragi/driver/tests/mod.rs', 'sumeragi/driver/tests/conformance.rs', 'conformance', 'sumeragi::driver::tests::conformance', (
        'f09_loss_duplication_reordering',
        'f13_crash_restart_churn',
        'f14_whole_cluster_restart',
        'f27_storage_faults_retried',
        'f29_cpu_flood',
        'f32_cluster_restart_lock_or_cqc',
        'f37_flagged_blocks',
        'o2_kill_at_each_write_completion',
        'long_write_failure_keeps_queues_bounded',
    )),
    ('native lane sample finalizer', 'sumeragi/lanes/step.rs', 'sumeragi/lanes/step/sample_owner_tests.rs', 'sample_owner_tests', 'sumeragi::lanes::step::sample_owner_tests', (
        'sample_finalizer_refusal_preserves_exact_source_and_retry_funds_only_suffix',
        'sample_finalizer_borrowed_lane_selection_preserves_boundaries_and_saturation',
    )),
    ('native lane sample publication', 'state.rs', 'state/lane_sample_owner_tests.rs', 'lane_sample_owner_tests', 'state::lane_sample_owner_tests', (
        'sample_world_rollback_publication_and_readers_retain_original_pool',
    )),
    ('native lane sample restore errors', 'snapshot/errors.rs', 'snapshot/errors/native_lane_sample_tests.rs', 'native_lane_sample_tests', 'snapshot::errors::native_lane_sample_tests', (
        'restore_lane_sample_refusal_keeps_original_typed_local_error',
    )),
    ('native stored body custody', 'sumeragi/block_store/body_read.rs', 'sumeragi/block_store/body_read_tests.rs', 'tests', 'sumeragi::block_store::body_read::tests', (
        'stored_body_retains_original_certificate_and_projected_payload_across_refusals',
        'absence_foreign_pool_and_corrupt_storage_are_separate_outcomes',
        'requested_hash_cannot_be_replaced_by_the_stored_certificate',
        'canonical_but_invalid_author_signature_never_becomes_available_custody',
        'stored_result_decode_refusal_retains_original_decoded_owners_and_retries',
        'malformed_result_preimage_remains_terminal_storage_corruption',
    )),
    ('native committed read custody', 'sumeragi/block_store/committed_read.rs', 'sumeragi/block_store/committed_read_tests.rs', 'tests', 'sumeragi::block_store::committed_read::tests', (
        'committed_read_returns_original_qc_backing_after_projection_refusal_and_retry',
        'body_only_read_releases_original_qc_witness_before_returning_ready',
        'committed_result_decode_refusal_keeps_original_read_slot_and_retries',
    )),
 )

def native_owner_stages(label=None, *, exclude=()):
    """Return explicitly selected static suites, independent of filesystem availability."""
    return tuple((coverage, tuple(prefix + "::" + leaf for leaf in leaves))
                 for coverage, _, _, _, prefix, leaves in NATIVE_CORE_TEST_OWNERS
                 if (label is None or coverage == label) and coverage not in exclude)


def rust_source_masker(root):
    """Use retained authenticated code in preparation, or explicit mutable development code."""
    if __name__ == "taira_captured_native_inventory":
        return _captured_mask_rust_comments
    helper = Path(root) / "scripts/formal/rust_text.py"
    namespace = {"__name__": "native_inventory_rust_text", "__file__": str(helper)}
    exec(compile(helper.read_bytes(), str(helper), "exec"), namespace)
    return namespace["mask_rust_comments"]


def validate_native_source_inventory(root, *, owners=NATIVE_CORE_TEST_OWNERS):
    """Require the complete reviewed native declaration census and parent registration."""
    mask = rust_source_masker(root)
    package = Path(root) / "crates/iroha_core/src"
    declarations = re.compile(r"#\[(?:tokio::)?test(?:\([^\]]*\))?\]\s*(?:#\[[^\]]*\]\s*)*(?:async\s+)?fn\s+([A-Za-z_]\w*)\s*\(")
    seen = set()
    for coverage, parent, source, module, prefix, expected in owners:
        for relative in (parent, source):
            if Path(relative).is_absolute() or ".." in Path(relative).parts:
                raise ValueError("native source owner leaves package: " + relative)
        if not expected or len(expected) != len(set(expected)):
            raise ValueError("native owner has empty or duplicated census: " + source)
        selected = {prefix + "::" + name for name in expected}
        if seen & selected:
            raise ValueError("native owners repeat exact test selectors")
        seen.update(selected)
        parent_text = (package / parent).read_text()
        parent_mask = mask(parent_text)
        registrations = list(re.finditer(r"\bmod\s+" + re.escape(module) + r"\s*([;{])", parent_mask))
        target = (package / source).resolve()
        bound = []
        for registration in registrations:
            if registration.group(1) == "{":
                # Included flat suites are bound inside this actual inline module body.
                start = registration.end()
                depth, end = 1, start
                while depth and end < len(parent_mask):
                    depth += (parent_mask[end] == "{") - (parent_mask[end] == "}")
                    end += 1
                for include in re.finditer(r'include!\("([^"\n]+)"\)', parent_text[start:end]):
                    offset = start + include.start()
                    if parent_mask[offset:offset + 8] == "include!":
                        bound.append((package / parent).parent / include.group(1))
            else:
                boundary = max(parent_mask.rfind(";", 0, registration.start()),
                               parent_mask.rfind("}", 0, registration.start())) + 1
                attributes = parent_text[boundary:registration.start()]
                paths = [match for match in re.finditer(r'#\[path\s*=\s*"([^"\n]+)"\]', attributes)
                         if parent_mask[boundary + match.start():boundary + match.start() + 6] == "#[path"]
                if len(paths) > 1:
                    raise ValueError("native test module source registration differs: " + prefix)
                if paths:
                    bound.append((package / parent).parent / paths[0].group(1))
                else:
                    base = (package / parent).parent
                    if Path(parent).name != "mod.rs":
                        base /= Path(parent).stem
                    bound.append(base / (module + ".rs"))
        if len(bound) != 1 or bound[0].resolve() != target:
            raise ValueError("native test module source registration differs: " + prefix)
        actual = tuple(declarations.findall(mask((package / source).read_text())))
        if set(actual) != set(expected) or len(actual) != len(expected):
            raise ValueError(f"native source census differs for {source}: missing={sorted(set(expected)-set(actual))}, extra={sorted(set(actual)-set(expected))}")
    return seen
