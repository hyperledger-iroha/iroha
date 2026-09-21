"""Offline checks for the early-gate runner; no Cargo or live inputs required."""

import contextlib
import errno
import fcntl
import hashlib
import stat
import struct
import importlib.util
import io
import json
import os
import re
import sys
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import MagicMock, patch


# Existing Mac census 852/1030, plus eight inspector, nine supervisor, five quiescence,
# seven generation-admission, nineteen reset integration, two PendingKura and one
# public-admission boundary control, 22 producer controls after two renames, and one
# early supervisor build-identity control, six typed status contention controls,
# three generated ledger/HTTP operator custody controls, and eleven finality witness
# and native inspection controls, four prebuilt portability/admission controls,
# three linked PendingKura owner recovery controls, 56 MV ownership controls,
# four typed lane-manifest controls, authenticated default genesis staging,
# canonical Taira stake-asset selection during no-config signing, and one
# inert PendingKura validation preview control, and six invocation-owned
# authenticated finality prefix controls, and two faucet-policy doctor controls.
# Four occupied-runtime and component-owned supervisor cleanup controls and two
# candidate funding-policy admission controls are mandatory, together with two
# content-bound journal/original-seed controls for metadata timestamp collisions.
# The configured initial-catalog/network control also runs before CLI checks.
# Two private-key fixture controls cover immutable-source signing and custody.
# Three explicit Torii listener controls preserve P2P and generated API ports.
# Five real execution publication controls retain witness, wire and State ownership.
# Twenty transaction-admission controls retain exact requests, deadlines and receipts.
# Twenty-two dispatcher controls preserve reversible upgrade custody and native preparation.
# Two native canary receipt controls retain unsuccessful evidence and exact proof bindings.
# Fifty-one native connection controls preserve transport, Queue and retained execution owners.
# Linux additionally
# selects OpenSSH, native worker identity and three Linux generation controls.
EXPECTED_BEACON_NETWORK_TEST = (
    'production_beacon_bootstrap::epoch_maintenance::production_epoch_supervisor_renews_and_resumes_after_owned_restart'
    if sys.platform == "linux" else
    'production_beacon_bootstrap::four_peer_fresh_custody_bootstrap_reaches_mandatory_pulse'
)
PLATFORM_REGRESSION_COUNT = 5 if sys.platform == "linux" else 0
EXPECTED_BASIC_REGRESSION_COUNT = 852 + 8 + 9 + 5 + 7 + 19 + 2 + 1 + 22 + 1 + 6 + 3 + 11 + 4 + 3 + 56 + 4 + 2 + 1 + 6 + 2 + 1 + 4 + 2 + 2 + 1 + 2 + 3 + 5 + 20 + 22 + 2 + 77 + 40 + PLATFORM_REGRESSION_COUNT
EXPECTED_REGRESSION_COUNT = 1030 + 8 + 9 + 5 + 7 + 19 + 2 + 1 + 22 + 1 + 6 + 3 + 11 + 4 + 3 + 56 + 4 + 2 + 1 + 6 + 2 + 1 + 4 + 2 + 2 + 1 + 2 + 3 + 5 + 20 + 22 + 2 + 77 + 40 + PLATFORM_REGRESSION_COUNT

SCRIPT = Path(__file__).with_name("taira_release_check.py")
if not SCRIPT.exists():
    SCRIPT = Path(__file__).resolve().parents[2] / "scripts/taira_release_check.py"
# Match direct script execution so deferred sibling imports work when this
# suite is run alone, without another test module modifying sys.path first.
sys.path.insert(0, str(SCRIPT.parent))
SPEC = importlib.util.spec_from_file_location("taira_release_check", SCRIPT)
assert SPEC and SPEC.loader
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


def isolate_shipping_fixture(case, *, keep_network_prerequisites=False):
    """Keep unrelated orchestration fixtures focused on their selected harnesses."""
    if not keep_network_prerequisites:
        prerequisites = patch.object(gate, "require_network_fixture_prerequisites")
        prerequisites.start()
        case.addCleanup(prerequisites.stop)
    inspector = patch.object(gate, "require_native_artifact_inspector")
    inspector.start()
    case.addCleanup(inspector.stop)
    audit = patch.object(gate, "shipping_harnesses", return_value=())
    audit.start()
    case.addCleanup(audit.stop)
    # These byte-sized fixtures test orchestration, not the operator's disk.
    # Capacity regressions override this observation with their exact boundary.
    capacity = patch.object(gate.shutil, "disk_usage", return_value=MagicMock(free=16 * 1024**3))
    capacity.start()
    case.addCleanup(capacity.stop)


def isolate_stage_fixture(stack, *, keep=()):
    """Clear maintained stage groups before a test supplies its synthetic census."""
    groups = {name: value for name, value in vars(gate).items()
              if name == "STAGES" or name.endswith("_STAGES")}
    if set(keep) - groups.keys():
        raise ValueError("synthetic fixture retained an unknown stage group")
    for name in groups:
        if name not in keep:
            stack.enter_context(patch.object(gate, name, ()))


class BeaconGateTests(unittest.TestCase):
    def test_actual_publication_controls_are_unique_and_focused_in_both_scopes(self):
        prefix = 'state::execution_publication_test_support::tests::'
        required = tuple(prefix + leaf for leaf in (
            'executed_genesis_and_successor_publish_real_finality_and_witnesses',
            'publication_rejects_an_overlay_from_another_state_before_durable_writes',
            'publication_rejects_changed_sealed_wire_with_the_same_header',
            'publication_requires_the_original_captured_witness',
            'publication_refuses_other_signed_genesis_validator_keys',
        ))
        for scope in gate.QUALIFICATION_SCOPES:
            selected = [leaf for _, leaves in gate.qualification_stages(scope)['core'] for leaf in leaves]
            startup = [leaf for _, leaves in gate.CORE_STARTUP_STAGES for leaf in leaves]
            for leaf in required:
                with self.subTest(scope=scope, regression=leaf):
                    self.assertEqual(selected.count(leaf), 1)
                    self.assertEqual(startup.count(leaf), 1)
                    focused = gate.focused_regression_stages(scope, ('core=' + leaf,))
                    self.assertEqual(tuple(focused), ('core',))
                    self.assertEqual([name for _, names in focused['core'] for name in names], [leaf])

    def test_prebuilt_portability_controls_are_required_and_focused_on_both_platforms(self):
        required = (
            "tests::program_absolute_prebuilt_override_does_not_require_checkout",
            "tests::program_discovery_requires_checkout_with_context",
            "tests::program_absolute_prebuilt_override_rejects_partial_release_identity",
            "tests::program_absolute_prebuilt_override_requires_active_release_checkout",
        )
        for platform in ("darwin", "linux"):
            spec = importlib.util.spec_from_file_location("prebuilt_portability_gate", gate.__file__)
            selected_gate = importlib.util.module_from_spec(spec)
            with patch.object(sys, "platform", platform):
                spec.loader.exec_module(selected_gate)
            for scope in selected_gate.QUALIFICATION_SCOPES:
                stages = selected_gate.qualification_stages(scope)["test-network"]
                names = [name for _, tests in stages for name in tests]
                for regression in required:
                    with self.subTest(platform=platform, scope=scope, regression=regression):
                        self.assertEqual(names.count(regression), 1)
                        focused = selected_gate.focused_regression_stages(scope, ("test-network=" + regression,))
                        self.assertEqual(tuple(focused), ("test-network",))
                        self.assertEqual([name for _, tests in focused["test-network"] for name in tests], [regression])
                        listing = "\n".join(name + ": test" for name in names if name != regression)
                        with self.assertRaisesRegex(selected_gate.CheckError, "required regressions missing"):
                            selected_gate.require_tests(listing, stages)

    def test_finality_prefix_controls_are_exact_required_and_focused_on_both_platforms(self):
        required = (
            'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_batches_and_pending_retries_authenticate_each_height_once',
            'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_rejects_changed_or_deleted_authenticated_disk_proof',
            'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_fresh_owner_reauthenticates_corrupted_disk_prefix',
            'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_invalid_successor_does_not_advance_retained_verifier',
            'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_publication_or_deadline_failure_does_not_commit_trial',
            'taira_dataspace_deploy::finality::authenticated_height::tests::deployment_prefix::deployment_prefix_lower_tip_keeps_frontier_and_rejects_conflicting_decision',
        )
        leaf = (SCRIPT.resolve().parents[1] / "crates/iroha_cli/src/taira_authenticated_height_tests.rs").read_text()
        self.assertIn("mod deployment_prefix {", leaf)
        for regression in required:
            self.assertEqual(leaf.count("    #[test]\n    fn " + regression.rsplit("::", 1)[1] + "()"), 1)
        for platform in ("darwin", "linux"):
            spec = importlib.util.spec_from_file_location("finality_prefix_gate", gate.__file__)
            selected_gate = importlib.util.module_from_spec(spec)
            with patch.object(sys, "platform", platform):
                spec.loader.exec_module(selected_gate)
            for scope in selected_gate.QUALIFICATION_SCOPES:
                stages = selected_gate.qualification_stages(scope)["cli"]
                names = [name for _, tests in stages for name in tests]
                for regression in required:
                    with self.subTest(platform=platform, scope=scope, regression=regression):
                        self.assertEqual(names.count(regression), 1)
                        focused = selected_gate.focused_regression_stages(scope, ("cli=" + regression,))
                        self.assertEqual(tuple(focused), ("cli",))
                        self.assertEqual([name for _, tests in focused["cli"] for name in tests], [regression])
                        listing = "\n".join(name + ": test" for name in names if name != regression)
                        with self.assertRaisesRegex(selected_gate.CheckError, "required regressions missing"):
                            selected_gate.require_tests(listing, stages)

    def test_finality_witness_and_native_inspection_controls_are_required_in_both_scopes(self):
        required = {
            'cli': (
                'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_accepts_independent_certificate_witnesses',
                'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_rejects_invalid_current_and_parent_witnesses',
                'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_rejects_signed_conflicting_decisions',
                'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_requires_authenticated_predecessor_for_alternate_witnesses',
            ),
            'core': (
                'kura::tests::block_store_read_only_finality_verifies_without_mutation',
                'kura::tests::block_store_read_only_finality_rejects_invalid_signature_and_binding',
                'kura::tests::block_store_read_only_finality_rejects_noncanonical_and_missing_records',
                'kura::tests::block_store_read_only_finality_rejects_unpublished_journal_boundary',
            ),
            'kagami': (
                'kura::tests::finality_inspection_rejects_invalid_height_before_store_access',
                'kura::tests::finality_inspection_failure_preserves_output_and_store',
                'kura::tests::finality_command_rejects_output_inside_store',
            ),
        }
        for platform in ("darwin", "linux"):
            spec = importlib.util.spec_from_file_location("finality_platform_gate", gate.__file__)
            selected_gate = importlib.util.module_from_spec(spec)
            with patch.object(sys, "platform", platform):
                spec.loader.exec_module(selected_gate)
            for scope in selected_gate.QUALIFICATION_SCOPES:
                selected = selected_gate.qualification_stages(scope)
                for harness, regressions in required.items():
                    names = [name for _, tests in selected[harness] for name in tests]
                    for regression in regressions:
                        with self.subTest(platform=platform, scope=scope, harness=harness, regression=regression):
                            self.assertEqual(names.count(regression), 1)
                            focused = selected_gate.focused_regression_stages(scope, (harness + "=" + regression,))
                            self.assertEqual(tuple(focused), (harness,))
                            self.assertEqual([name for _, tests in focused[harness] for name in tests], [regression])
                            listing = "\n".join(name + ": test" for name in names if name != regression)
                            with self.assertRaisesRegex(selected_gate.CheckError, "required regressions missing"):
                                selected_gate.require_tests(listing, selected[harness])

    def test_status_contention_controls_are_exact_required_and_focused_on_both_platforms(self):
        required = {
            'core': (
                'state::telemetry_status::tests::status_source_busy_is_distinct_from_changed_or_invalid_journal',
            ),
            'torii-shared': (
                'status::failure::tests::status_failure_codes_are_exact_and_distinct',
                'status::failure::tests::status_failure_codes_do_not_accept_unclassified_input_as_a_reason',
            ),
            'torii-unit': (
                'routing::status_failure_reason_tests::snapshot_failure_reasons_match_json_norito_and_header',
            ),
            'client': (
                'client::status_http_tests::status_unavailable_reasons_are_safe_in_errors_and_do_not_trigger_retries',
                'client::status_http_tests::status_unavailable_rejects_missing_unknown_invalid_and_duplicate_reason_headers',
            ),
        }
        for platform in ("darwin", "linux"):
            spec = importlib.util.spec_from_file_location("status_platform_gate", gate.__file__)
            self.assertIsNotNone(spec)
            self.assertIsNotNone(spec.loader)
            selected_gate = importlib.util.module_from_spec(spec)
            with patch.object(sys, "platform", platform):
                spec.loader.exec_module(selected_gate)
            for scope in selected_gate.QUALIFICATION_SCOPES:
                selected = selected_gate.qualification_stages(scope)
                for harness, regressions in required.items():
                    names = [name for _, tests in selected[harness] for name in tests]
                    for regression in regressions:
                        with self.subTest(platform=platform, scope=scope, harness=harness, regression=regression):
                            self.assertEqual(names.count(regression), 1)
                            focused = selected_gate.focused_regression_stages(scope, (harness + "=" + regression,))
                            self.assertEqual(tuple(focused), (harness,))
                            self.assertEqual([name for _, tests in focused[harness] for name in tests], [regression])
                            listing = "\n".join(name + ": test" for name in names if name != regression)
                            with self.assertRaisesRegex(selected_gate.CheckError, "required regressions missing"):
                                selected_gate.require_tests(listing, selected[harness])
                            if harness == "core":
                                startup = [name for _, tests in selected_gate.CORE_STARTUP_STAGES for name in tests]
                                self.assertEqual(startup.count(regression), 1)

    def test_paid_genesis_authority_and_public_failure_controls_precede_network_in_both_scopes(self):
        required = (
            'dataspace_deploy_cli::signed_genesis_validator_mapping_preserves_runtime_accounts',
            'dataspace_deploy_cli::phase_failure_summary_excludes_signed_payloads',
        )
        expensive = EXPECTED_BEACON_NETWORK_TEST
        observation = [leaf for _, leaves in gate.NETWORK_OBSERVATION_STAGES for leaf in leaves]
        for scope in gate.QUALIFICATION_SCOPES:
            selected = [leaf for _, leaves in gate.qualification_stages(scope)['network'] for leaf in leaves]
            for leaf in required:
                with self.subTest(scope=scope, regression=leaf):
                    self.assertEqual(selected.count(leaf), 1)
                    self.assertEqual(observation.count(leaf), 1)
                    self.assertLess(selected.index(leaf), selected.index(expensive))
                    focused = gate.focused_regression_stages(scope, ('network=' + leaf,))
                    self.assertEqual(tuple(focused), ('network',))
                    self.assertEqual([name for _, names in focused['network'] for name in names], [leaf])

    def test_merge_beacon_composition_controls_are_unique_and_focused_in_both_scopes(self):
        required = (
            'state::tests::autonomous_merge_beacon_composition_preserves_certified_roots_and_commits_once',
            'state::tests::autonomous_merge_beacon_composition_rejects_invalid_effects_and_post_seal_drift',
        )
        for scope in gate.QUALIFICATION_SCOPES:
            selected = [name for _, names in gate.qualification_stages(scope)["core"] for name in names]
            startup = [name for _, names in gate.CORE_STARTUP_STAGES for name in names]
            for name in required:
                with self.subTest(scope=scope, regression=name):
                    self.assertEqual(selected.count(name), 1)
                    self.assertEqual(startup.count(name), 1)
                    focused = gate.focused_regression_stages(scope, ("core=" + name,))
                    self.assertEqual(tuple(focused), ("core",))
                    self.assertEqual([leaf for _, names in focused["core"] for leaf in names], [name])

    def test_mandatory_beacon_requires_real_work_before_activation_in_both_scopes(self):
        required = (
            'sumeragi::v2_candidate::tests::proposal_work_gate_rejects_beacon_pulse_only',
            'sumeragi::v2_candidate::tests::proposal_work_gate_preserves_non_beacon_effects',
            'sumeragi::v2_candidate::tests::mandatory_beacon_wait_requires_independent_work',
            'sumeragi::v2_candidate::tests::mandatory_beacon_wait_releases_same_queue_prefix_for_retry',
            'beacon::tests::threshold_beacon_deferred_mandatory_height_stays_idle_until_real_work',
            'beacon::tests::threshold_beacon_live_v2_producer_is_bound_restartable_and_persists_effect',
        )
        for scope in gate.QUALIFICATION_SCOPES:
            stages = gate.qualification_stages(scope)
            selected = [name for _, names in stages["core"] for name in names]
            startup = [name for _, names in gate.CORE_STARTUP_STAGES for name in names]
            for name in required:
                with self.subTest(scope=scope, regression=name):
                    self.assertEqual(selected.count(name), 1)
                    self.assertEqual(startup.count(name), 1)
                    focused = gate.focused_regression_stages(scope, ("core=" + name,))
                    self.assertEqual(tuple(focused), ("core",))
                    self.assertEqual([leaf for _, names in focused["core"] for leaf in names], [name])

    def test_exact_height_lifecycle_controls_are_unique_and_focused_in_both_scopes(self):
        required = {
            'cli': (
                'tests::fee_quote_signing_preserves_explicit_ordinary_payload_and_expiry',
                'taira_public_reset::host::beacon::tests::beacon_install_envelope_requires_ordinary_exact_certificate',
                'taira_public_reset::public_inputs::tests::beacon_bootstrap_window_reserves_real_queue_plan_canary_and_install',
            ),
            'daemon': (
                'beacon_bootstrap::tests::bootstrap_records_observed_height_jumps_and_rejects_pulse_collision',
            ),
            'torii-unit': (
                'tests_runtime_handlers::lifecycle_ordinary_ingress_accepts_exact_quorum_and_preserves_wire_identity',
                'tests_runtime_handlers::lifecycle_ordinary_ingress_rejects_general_and_mixed_transactions',
                'tests_runtime_handlers::lifecycle_ordinary_ingress_rejects_invalid_certificate_authority',
                'tests_runtime_handlers::lifecycle_ordinary_ingress_requires_authenticated_parent_and_global_route',
            ),
        }
        for scope in gate.QUALIFICATION_SCOPES:
            selected = gate.qualification_stages(scope)
            for harness, regressions in required.items():
                names = [name for _, tests in selected[harness] for name in tests]
                for regression in regressions:
                    with self.subTest(scope=scope, harness=harness, regression=regression):
                        self.assertEqual(names.count(regression), 1)
                        focused = gate.focused_regression_stages(scope, (harness + "=" + regression,))
                        self.assertEqual(tuple(focused), (harness,))
                        self.assertEqual([name for _, tests in focused[harness] for name in tests], [regression])

    def test_initial_catalog_control_is_selected_once_before_cli_in_both_scopes(self):
        name = "tests_runtime_handlers::configured_catalog_fixture_binds_initial_geometry_and_explicit_network"
        startup = [leaf for _, names in gate.TORII_STARTUP_STAGES for leaf in names]
        self.assertEqual(startup.count(name), 1)
        for scope in gate.QUALIFICATION_SCOPES:
            with self.subTest(scope=scope):
                selected = gate.qualification_stages(scope)["torii-unit"]
                self.assertEqual([leaf for _, names in selected for leaf in names].count(name), 1)
                focused = gate.focused_regression_stages(scope, ("torii-unit=" + name,))
                self.assertEqual(tuple(focused), ("torii-unit",))
                self.assertEqual([leaf for _, names in focused["torii-unit"] for leaf in names], [name])

    def test_controller_beacon_authority_and_public_builder_controls_are_exact_in_both_scopes(self):
        required = (
            'tests::authorized_transaction_lifetime_uses_exact_creation_and_preserves_shorter_ttl',
            'tests::authorized_transaction_lifetime_rejects_empty_window_and_missing_ttl',
            'taira::tests::final_canary_expired_window_rejects_before_fee_quote_or_dispatch',
            'taira::tests::final_canary_submit_uses_original_deadline_after_initial_read_and_post',
            'taira::tests::final_canary_submit_verifies_exact_proof_without_replaying_post',
            'taira_public_reset::host::beacon::tests::signed_beacon_plan_binds_roster_seats_and_exact_final_units',
            'taira_public_reset::host::beacon::tests::beacon_config_projection_changes_only_exact_provider_fields',
            'taira_public_reset::host::beacon::tests::lost_beacon_ceremony_cannot_restart_or_repeat_committed_canaries',
            'taira_public_reset::host::beacon::tests::beacon_owned_child_deadline_retains_private_attempt',
            'taira_public_reset::host::tests::beacon_activation_barrier_preserves_pre_ready_bootstrap_and_blocks_later_mutations',
            'taira_public_reset::executor_model::tests::beacon_submitted_continuation_retains_exact_host_cursor_and_excludes_ledger_work',
            'taira_public_reset::executor_model::tests::beacon_continuation_outcome_cannot_reclassify_submitted_ledger_transaction',
            'taira_public_reset::host::beacon::tests::beacon_successful_early_child_exit_cannot_authorize_another_operation',
            'taira_public_reset::host::beacon::tests::beacon_unit_publication_preserves_completed_inode_and_rejects_substitution',
            'taira_public_reset::inputs::tests::topology_intent_forbids_generated_pins_and_plans',
            'taira_public_reset::public_inputs::tests::beacon_public_preparation_derives_native_nonce_bound_seats_and_rejects_substitution',
            'taira_public_reset::public_inputs::tests::public_bundle_requires_authenticated_raw_manifest_without_four_file_fallback',
            'taira_public_reset::public_inputs::tests::public_bundle_derives_canary_from_topology_intent_without_key_file',
            'taira_public_reset::host::tests::host_receipt_names_cover_every_action_and_artifact_role',
            'taira_public_reset::host::tests::recovery_intent_exposes_every_ordered_child_mutation',
            'taira_public_reset::host::tests::core_testnet_scope_preserves_baseline_recovery_and_host_plan',
            'taira_public_reset::executor_model::tests::recovery_args_accept_identical_forward_inputs_without_admitting_unused_paths',
            'taira_public_reset::inputs::tests::assembler_rejects_incomplete_topology_before_reading_runtime_inputs',
            'taira_public_reset::inputs::tests::aggregate_timeout_budget_rejects_assembly_and_authorization_before_input_or_custody_reads',
            'taira_public_reset::public_inputs::tests::derives_native_genesis_identity_and_exact_canary_request',
            'taira_public_reset::public_inputs::tests::rejects_wrong_network_key_and_resultless_genesis',
            'taira_public_reset::public_inputs::tests::rejects_noncanonical_identity_and_non_ed25519_canary',
            'taira_public_reset::public_inputs::tests::publishes_complete_public_bundle_and_reuses_identical_request',
            'taira_public_reset::public_inputs::tests::refuses_changed_bundle_and_never_overwrites_existing_output',
            'taira_public_reset::public_inputs::tests::rejects_symlink_input_and_partial_or_surplus_output',
            'taira_public_reset::public_inputs::tests::cli_public_input_preparation_never_accepts_private_credentials',
            'taira_public_reset::deployment_profile::tests::deployment_profile_binds_native_genesis_and_ordered_inventory_peers',
            'taira_public_reset::deployment_profile::tests::deployment_profile_rejects_genesis_artifact_peer_and_slot_substitution',
            'tests::taira_public_reset_local_inputs_require_a_dedicated_operator_key',
        )
        for scope in gate.QUALIFICATION_SCOPES:
            selected = [name for _, names in gate.qualification_stages(scope)["cli"] for name in names]
            for regression in required:
                with self.subTest(scope=scope, regression=regression):
                    self.assertEqual(selected.count(regression), 1)
                    focused = gate.focused_regression_stages(scope, ("cli=" + regression,))
                    self.assertEqual(tuple(focused), ("cli",))
                    self.assertEqual([name for _, names in focused["cli"] for name in names], [regression])
        self.assertEqual(gate.HARNESS_TARGETS["cli"][3], ["-p", "iroha_cli", "--bin", "iroha"])

    def test_real_beacon_fixture_replaces_clean_client_and_keeps_root_check_before_startup(self):
        real = EXPECTED_BEACON_NETWORK_TEST
        root = "production_beacon_bootstrap::production_beacon_fixture_root_rejects_git_symlink_and_shared_custody"
        exact_height = "production_beacon_bootstrap::production_beacon_exact_height_wait_preserves_retained_tip"
        seam = "taira_runtime_signer::tests::production_beacon_fixture_guard_keeps_exact_core_only_taira_identity"
        self.assertEqual([name for _, names in gate.BEACON_NETWORK_STAGES for name in names], [real])
        self.assertIn(root, [name for _, names in gate.NETWORK_OBSERVATION_STAGES for name in names])
        self.assertIn(exact_height, [name for _, names in gate.NETWORK_OBSERVATION_STAGES for name in names])
        for scope in gate.QUALIFICATION_SCOPES:
            selected = gate.qualification_stages(scope)
            network = [name for _, names in selected["network"] for name in names]
            self.assertEqual(network.count(real), 1)
            self.assertEqual(network.count(root), 1)
            self.assertEqual(network.count(exact_height), 1)
            self.assertLess(network.index(exact_height), network.index(real))
            focused = gate.focused_regression_stages(scope, ("network=" + exact_height,))
            self.assertEqual(tuple(focused), ("network",))
            self.assertEqual([name for _, names in focused["network"] for name in names], [exact_height])
            self.assertNotIn("dataspace_deploy_cli::clean_client_deploys_paid_dataspace_once_with_four_peer_finality", network)
            for retired in (
                "four_peer_universal_public_transaction_sequence_reaches_applied",
                "four_peer_multiroute_public_transaction_sequence_reaches_applied",
                "runtime_catalog_transition::four_peer_committed_catalog_transition_preserves_history_and_replay",
            ):
                self.assertNotIn(retired, network)
            self.assertEqual(selected["network"], gate.NETWORK_OBSERVATION_STAGES + gate.BEACON_NETWORK_STAGES)
            self.assertEqual([name for _, names in selected["daemon"] for name in names].count(seam), 1)

    def test_beacon_setup_and_custody_run_once_in_startup_for_every_scope(self):
        for scope in gate.QUALIFICATION_SCOPES:
            selected = gate.qualification_stages(scope)
            for harness, required, startup in (
                ("core", gate.CORE_BEACON_STAGES, gate.CORE_STARTUP_STAGES),
                ("daemon", gate.DAEMON_BEACON_STAGES, gate.DAEMON_STARTUP_STAGES),
                ("torii-unit", gate.TORII_BEACON_STAGES, gate.TORII_STARTUP_STAGES),
            ):
                names = [name for _, tests in selected[harness] for name in tests]
                for stage in required:
                    self.assertIn(stage, startup)
                    for name in stage[1]:
                        with self.subTest(scope=scope, harness=harness, regression=name):
                            self.assertEqual(names.count(name), 1)


class SyntheticStageInventoryTests(unittest.TestCase):
    def test_stage_reset_covers_new_groups_preserves_explicit_groups_and_restores_inventory(self):
        sentinel = (("future stage", ("future_test",)),)
        with patch.object(gate, "FUTURE_STAGES", sentinel, create=True):
            original = {name: value for name, value in vars(gate).items()
                        if name == "STAGES" or name.endswith("_STAGES")}
            with contextlib.ExitStack() as stack:
                isolate_stage_fixture(stack, keep=("PROOF_STAGES", "PROOF_FLOW_STAGES"))
                for name, value in original.items():
                    if name in {"PROOF_STAGES", "PROOF_FLOW_STAGES"}:
                        self.assertIs(getattr(gate, name), value)
                    else:
                        self.assertEqual(getattr(gate, name), ())
                self.assertEqual(gate.TORII_SHARED_STAGES, ())
                self.assertEqual(gate.FUTURE_STAGES, ())
                selected = gate.qualification_stages("full")
                self.assertEqual({name for name, stages in selected.items() if stages}, {"proof", "proof-flows"})
            for name, value in original.items():
                self.assertIs(getattr(gate, name), value)

    def test_unknown_retained_stage_fails_before_mutating_inventory(self):
        original = gate.STAGES
        with contextlib.ExitStack() as stack, self.assertRaisesRegex(ValueError, "unknown stage"):
            isolate_stage_fixture(stack, keep=("UNREVIEWED_STAGES",))
        self.assertIs(gate.STAGES, original)


class FixtureCopies(dict):
    """Only the mapping/context surface when a test mocks artifact compilation."""
    def __init__(self, value):
        super().__init__({name: value for name in gate.HARNESS_TARGETS} if isinstance(value, str) else value)
    def __enter__(self):
        return self
    def __exit__(self, *args):
        return False
    def release(self, selection):
        pass


class BasicReleaseQualificationTests(unittest.TestCase):
    def setUp(self):
        metadata = patch.object(gate, "check_test_harnesses")
        self.test_metadata = metadata.start()
        self.addCleanup(metadata.stop)
        prerequisites = patch.object(gate, "require_network_fixture_prerequisites")
        prerequisites.start()
        self.addCleanup(prerequisites.stop)
        inspector = patch.object(gate, "require_native_artifact_inspector")
        inspector.start()
        self.addCleanup(inspector.stop)

    def test_mv_ownership_census_is_exact_required_and_focused_on_both_platforms(self):
        library_groups = {
            "allocation::tests::": 6,
            "release::tests::": 11,
            "cell::charged_allocation_tests::": 7,
            "storage::publication_tests::": 9,
            "storage::detached_tests::": 9,
        }
        for platform in ("darwin", "linux"):
            spec = importlib.util.spec_from_file_location("mv_ownership_gate", gate.__file__)
            selected_gate = importlib.util.module_from_spec(spec)
            with patch.object(sys, "platform", platform):
                spec.loader.exec_module(selected_gate)
            self.assertEqual(selected_gate.MV_OWNERSHIP_HARNESSES, ("mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread"))
            for scope in selected_gate.QUALIFICATION_SCOPES:
                selected = selected_gate.qualification_stages(scope)
                library = [test for _, tests in selected["mv"] for test in tests]
                self.assertEqual(len(library), 42)
                for prefix, count in library_groups.items():
                    self.assertEqual(sum(name.startswith(prefix) for name in library), count)
                for harness, count in (("mv", 42), ("mv-ebr", 5), ("mv-map", 9), ("mv-admitted-map", 21), ("concread", 17)):
                    names = [test for _, tests in selected[harness] for test in tests]
                    self.assertEqual(len(names), count)
                    self.assertEqual(len(set(names)), count)
                    for regression in names:
                        with self.subTest(platform=platform, scope=scope,
                                          harness=harness, regression=regression):
                            focused = selected_gate.focused_regression_stages(
                                scope, (harness + "=" + regression,))
                            self.assertEqual(tuple(focused), (harness,))
                            self.assertEqual([test for _, tests in focused[harness] for test in tests],
                                             [regression])
                            listing = "\n".join(name + ": test" for name in names if name != regression)
                            with self.assertRaisesRegex(selected_gate.CheckError, "required regressions missing"):
                                selected_gate.require_tests(listing, selected[harness])

    def test_admitted_map_and_concread_select_the_actual_closed_source_test_census(self):
        root = Path(gate.__file__).resolve().parents[1]
        names = lambda text: re.findall(r"#\[test\]\s*fn\s+(\w+)", text)
        admitted = names((root / "crates/mv/tests/admitted_map_custody.rs").read_text())
        admission_source = (root / "vendor/concread/src/bptree/admission_tests.rs").read_text()
        ordinary, writer = admission_source.split("mod writer_start {", 1)
        checkpoint = names((root / "vendor/concread/src/internals/bptree/checkpoint_tests.rs").read_text())
        expected = {
            "mv-admitted-map": admitted,
            "concread": [*("bptree::admission::tests::" + name for name in names(ordinary)),
                         *("bptree::admission::tests::writer_start::" + name for name in names(writer)),
                         *("internals::bptree::cursor::checkpoint::tests::" + name for name in checkpoint)],
        }
        self.assertEqual((len(admitted), len(names(ordinary)), len(names(writer)), len(checkpoint)), (21, 7, 6, 4))
        self.assertIn('#[path = "admission_tests.rs"]\nmod tests;',
                      (root / "vendor/concread/src/bptree/admission.rs").read_text())
        self.assertIn('#[path = "checkpoint.rs"]\nmod checkpoint;',
                      (root / "vendor/concread/src/internals/bptree/cursor.rs").read_text())
        self.assertIn('#[path = "checkpoint_tests.rs"]\nmod tests;',
                      (root / "vendor/concread/src/internals/bptree/checkpoint.rs").read_text())
        for scope in gate.QUALIFICATION_SCOPES:
            selected = gate.qualification_stages(scope)
            for harness, actual_source_names in expected.items():
                self.assertEqual([name for _, names in selected[harness] for name in names], actual_source_names)
        self.assertEqual(gate.HARNESS_TARGETS["mv-admitted-map"][3], ["-p", "mv", "--test", "admitted_map_custody"])
        self.assertEqual(gate.HARNESS_TARGETS["concread"][3], ["-p", "concread", "--lib"])

    def test_wallet_extraction_preserves_http_cpu_and_canonical_policy_coverage(self):
        expected = {
            "cli": "taira::tests::faucet_preparation_deadline_stops_http_before_dispatch",
            "wallet": "faucet_pow::resource_tests::faucet_preparation_deadline_stops_cpu_work_before_dispatch",
            "client": "account_bootstrap::tests::faucet_discovery_requires_exact_canonical_v1_fields",
            "data-model": "asset::id::tests::asset_definition_id_requires_exact_canonical_text_across_decoders",
        }
        for scope in gate.QUALIFICATION_SCOPES:
            stages = gate.qualification_stages(scope)
            for harness, name in expected.items():
                self.assertEqual([test for _, names in stages[harness] for test in names].count(name), 1)
                self.assertEqual(tuple(gate.focused_regression_stages(scope, (harness + "=" + name,))), (harness,))
                listing = "\n".join(test + ": test" for _, names in stages[harness] for test in names if test != name)
                with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                    gate.require_tests(listing, stages[harness])
            for stale in ("faucet_preparation_deadline_stops_http_and_cpu_work_before_dispatch",
                          "doctor_faucet_policy_requires_exact_canonical_v1_fields"):
                with self.assertRaisesRegex(gate.CheckError, "not selected"):
                    gate.focused_regression_stages(scope, ("cli=taira::tests::" + stale,))
        self.assertEqual(gate.HARNESS_TARGETS["wallet"][3], ["-p", "iroha_wallet", "--lib"])

    def test_faucet_policy_seven_leaves_are_selected_in_both_scopes(self):
        expected = {'torii': ('accounts_faucet::accounts_faucet_policy_exposes_exact_public_configuration', 'accounts_faucet::accounts_faucet_policy_resolves_configured_asset_alias', 'accounts_faucet::accounts_faucet_policy_preserves_disabled_forbidden_response'), 'torii-unit': ('mcp::tests::faucet_policy_tool_is_read_only_and_runtime_gated', 'mcp::tests::faucet_policy_tool_dispatches_only_get_without_body', 'openapi::tests::faucet_policy_schema_is_exact_public_discovery'), 'torii-shared': ('route_catalog::tests::account_faucet_policy_is_public_read_only_discovery',)}
        for scope in gate.QUALIFICATION_SCOPES:
            selected = gate.qualification_stages(scope)
            for harness, names in expected.items():
                actual = [name for _, stage in selected[harness] for name in stage]
                for name in names:
                    self.assertEqual(actual.count(name), 1)
                    self.assertEqual(tuple(gate.focused_regression_stages(scope, (harness + "=" + name,))), (harness,))
        self.assertEqual(gate.HARNESS_TARGETS["torii-shared"][3], ["-p", "iroha_torii_shared", "--lib"])


    def test_both_scopes_require_geometry_writer_and_profile_recovery(self):
        required = {
            "client": (
                "blocking::tests::borrowed_async_client_reuses_keepalive_connection_between_blocking_calls",
                "blocking::tests::background_tasks_progress_with_a_clone_and_cancel_after_final_owner_drop",
                "client::evidence_http_tests::bridge_finality_attestation_reader_preserves_only_bound_typed_tip_progress",
                "client::evidence_http_tests::bridge_finality_attestation_reader_rejects_malformed_or_unbound_tip_progress",
                "client::evidence_http_tests::bridge_finality_attestation_reader_rejects_untyped_or_noncanonical_progress_http",
                "client::tests::decode_parameters_response_parses_json_payload",
                "client::evidence_http_tests::get_transaction_status_response_global_sets_global_scope",
                "client::evidence_http_tests::pipeline_status_404_returns_none_from_exact_global_query",
                "client::transaction_wait_tests::transaction_wait_timeout_identifies_the_exact_pending_transaction",
                "client::transaction_wait_tests::wait_for_transaction_applied_rejects_fixed_failures",
                "client::transaction_wait_tests::transaction_wait_zero_timeout_never_dispatches_an_initial_read",
                "client::transaction_wait_tests::transaction_wait_unrepresentable_deadline_fails_before_dispatch",
                "client::transaction_wait_tests::transaction_wait_expired_context_deadline_cannot_be_extended",
                "client::transaction_wait_tests::transaction_wait_late_http_status_is_unresolved_in_both_transports",
                "client::transaction_wait_tests::transaction_wait_retries_spend_one_remaining_http_budget",
                "client::transaction_wait_tests::transaction_wait_outcome_admission_rechecks_deadline_after_decoding",
                "client::transaction_wait_tests::transaction_wait_async_deadline_retires_the_pending_status_future",
                "client::tests::typed_account_alias_reads_map_not_found_to_none",
            ),
            "torii-unit": (
                'tests_runtime_handlers::canonical_outcome_releases_state_snapshot_before_kura_authentication',
                'tests_runtime_handlers::canonical_outcome_preserves_exact_committed_rejection',
                'tests_runtime_handlers::canonical_outcome_absent_membership_never_authenticates',
                'tests_runtime_handlers::canonical_outcome_accepts_unrelated_state_append_after_authentication',
                'tests_runtime_handlers::canonical_outcome_rejects_removed_membership_after_authentication',
                'tests_runtime_handlers::canonical_outcome_rejects_rebound_membership_after_authentication',
                'tests_runtime_handlers::canonical_outcome_rejects_replaced_journal_after_authentication',
                'tests_runtime_handlers::canonical_outcome_rejects_missing_journal_after_authentication',
                'tests_runtime_handlers::canonical_outcome_rejects_result_substitution_under_the_same_header_hash',
                'tests_runtime_handlers::canonical_outcome_authentication_error_cannot_fall_back_to_terminal_cache',
                "routing::bridge_finality_attestation_progress_tests::exact_tip_snapshot_races_are_bound_negotiated_progress",
                "routing::bridge_finality_attestation_progress_tests::proof_identity_and_signature_failures_are_never_tip_progress",
                "routing::bridge_finality_attestation_progress_tests::canonical_boundary_keeps_only_valid_tip_progress_status_and_code",
                "routing::bridge_finality_attestation_progress_tests::invalid_height_progress_shapes_remain_fixed_errors",
                "openapi::tests::finality_attestation_tip_progress_openapi_matches_native_bindings",
                "openapi::tests::compact_finality_app_contracts::bridge_finality_operations_describe_durable_v2_evidence",
                "tests_runtime_handlers::pipeline_status_global_read_skips_non_terminal_local_cache",
                "torii_routed_read_tests::pipeline_status_fanout_requires_exact_scoped_absence",
                "openapi::tests::pipeline_status_openapi_exposes_only_the_exact_first_release_scope",
                "mcp::tests::canonical_paths_and_status::applied_wait_status_poll_accepts_only_exact_200_or_404",
                "tests::alias_error_envelopes_preserve_reports_and_exact_absence_through_middleware",
                "tests::alias_account_absence_requires_complete_scoped_fanout",
                "openapi::tests::alias_errors_openapi_match_native_reports_and_bound_absence",
            ),
            "torii-shared": (
                "bridge_finality::tests::tip_mismatch_requires_exact_selector_and_real_height_progress",
                "tests::pipeline_transaction_status_roundtrip_is_status_only",
                "aliases::tests::alias_error_details_roundtrip_and_reject_unknown_fields",
            ),
            "cli": (
                'taira_dataspace_deploy::profile::tests::retained_profile_export_uses_native_trust_and_exact_input_hashes',
                'taira_dataspace_deploy::profile::tests::retained_profile_export_rejects_unbound_or_malformed_public_inputs',
                'taira_dataspace_deploy::profile::tests::retained_profile_export_rejects_changed_linked_and_unsafe_files',
                'taira_dataspace_deploy::profile::tests::retained_profile_export_dispatch_rejects_credential_and_transaction_globals',
                "taira_dataspace_deploy::tests::saved_apply_emits_report_before_rejecting_incomplete_success",
                "taira_dataspace_deploy::tests::saved_report_preserves_output_failure",
                "taira_dataspace_deploy::finality::tests::deployment_attestation_progress_retries_only_exact_sdk_type",
                "taira_dataspace_deploy::finality::tests::deployment_attestation_progress_joins_all_peers_and_preserves_fixed_errors",
                "taira_dataspace_deploy::tests::journal_rejects_links_replacement_and_incomplete_records",
                "taira_dataspace_deploy::finality::tests::deployment_peer_reads_overlap_and_preserve_input_order",
                "taira_dataspace_deploy::finality::tests::deployment_peer_reads_reject_non_four_cardinality_before_dispatch",
                "taira_dataspace_deploy::finality::tests::deployment_peer_reads_join_all_workers_and_report_first_error",
                "taira_dataspace_deploy::finality::tests::deployment_peer_reads_inherit_configured_address_profile",
                "taira_dataspace_deploy::finality::tests::deployment_peer_reads_recover_worker_panic_after_joining_all",
                "taira_dataspace_deploy::finality::tests::deployment_carrier_results_require_exact_bytes_before_publication",
                "taira_dataspace_deploy::tests::status_requires_exact_global_and_peer_state_applied",
                "taira_dataspace_deploy::finality::tests::deployment_peer_progress_requires_valid_complete_status_pair",
                "taira_dataspace_deploy::finality::tests::deployment_peer_progress_retries_only_pending_or_newer_carrier",
                "taira_dataspace_deploy::finality::tests::deployment_peer_progress_never_masks_fixed_worker_errors",
                "taira_dataspace_deploy::tests::saved_commands_require_positive_budget_and_default_to_three_minutes",
                "taira_dataspace_deploy::tests::saved_zero_budget_stops_before_journal_or_client_access",
                "taira_dataspace_deploy::tests::expired_operation_never_observes_or_starts_completion",
                "taira_dataspace_deploy::tests::apply_observes_pending_until_applied_without_reentering_dispatch",
                "taira_dataspace_deploy::tests::status_observes_once_and_terminal_apply_does_not_retry",
                "taira_dataspace_deploy::tests::phase_deadline_rejects_late_applied_and_clips_pending_sleep",
                "taira_dataspace_deploy::tests::completion_retries_only_explicit_sync_progress_and_status_is_one_attempt",
                "taira_dataspace_deploy::tests::completion_deadline_rejects_a_late_success",
            ),
            "core": (
                'kura::tests::autonomous_latest_snapshot_reuses_validated_current_cursor',
                'kura::tests::autonomous_completion_selected_view_rejects_corruption_and_foreign_suffix',
                'kura::tests::certified_lane_block_read_rejects_qc_signature_mismatch',
                'kura::tests::certified_lane_block_read_rejects_qc_body_mismatch',
                'sumeragi::v2_runner::tests::open_preflight_batch_services_queued_prepare_and_commit_before_reaudit',
                'sumeragi::v2_runner::tests::open_preflight_batch_preserves_budget_completion_yield_and_errors',
                'sumeragi::v2_runner::tests::open_preflight_batch_does_not_admit_global_traffic_as_lane_recovery',
                'sumeragi::v2_lane_work::tests::historical_autonomous_hydration_replaces_same_slot_conflict_at_capacity',
                'sumeragi::v2_lane_work::tests::historical_autonomous_hydration_preserves_conflicting_quorum_at_capacity',
                'sumeragi::v2_lane_work::tests::finalized_carrier_nonmember_cache_invalid_commit_certificate_rolls_back_hydration',
                'sumeragi::v2_lane_work::tests::global_validator_outside_lane_committee_uses_canonical_replica_for_rollover',
                'kura::tests::certified_lane_block_rejects_foreign_active_dataspace',
                'kura::tests::autonomous_completion_missing_view_state_keeps_full_payload_validation',
                'kura::tests::autonomous_completion_selected_view_validates_artifact_once',
                'sumeragi::v2_runner::tests::terminal_finalization_limits_open_ingress_to_lane_preflight_before_the_finite_closed_drain',
                "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_snapshot_tracks_live_depth_and_oldest_age",
                "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_checked_dequeue_freezes_one_physical_cut_per_occurrence",
                "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_closed_drained_cut_rejects_each_stale_lane_account",
                "sumeragi::v2_runner::tests::finalized_closed_prefix_retires_historical_lane_certificate_without_adapter_admission",
                "sumeragi::v2_worker::tests::prepared_historical_body_capacity_recovers_from_applied_finality_without_peer_delivery",
                "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::complete_tip_terminal_apply_store_join_rejects_store_drift",
                "sumeragi::v2_runner::tests::synthesized_durable_rollover_contract_allows_successor_after_dead_target_handoff",
                "kura::tests::consensus_certificate_read_rejects_occupied_corruption_without_repair",
                "sumeragi::v2_lane_work::tests::same_proposal_shortcut_rejects_unvalidated_certificate_variants",
                "kura::tests::canonical_autonomous_replica_corruption_and_wrong_context_fail_closed",
                "sumeragi::v2_lane_work::tests::canonical_lane_recovery_restores_handoff_after_losing_carrier_retirement",
                "queue::router::alias_registry_routing_tests::alias_registry_routing_paid_post_genesis_dataspace_domain_and_renewal",
                "queue::router::alias_registry_routing_tests::alias_registry_routing_is_independent_of_height_and_catalog",
                "queue::router::alias_registry_routing_tests::alias_registry_routing_nested_walkers_use_universal_registry",
                "queue::router::alias_registry_routing_tests::alias_registry_routing_does_not_bypass_id_owner_quote_or_catalog_guards",
                "queue::router::alias_registry_routing_tests::alias_registry_routing_keeps_real_private_participants_in_mixed_transactions",
                "queue::router::alias_registry_routing_tests::alias_registry_routing_cold_replay_with_expanded_catalog_preserves_paid_bootstrap",
                "queue::router::tests::alias_registry_routing_is_unconditional_for_queue_and_replay",
                "kura::tests::startup_replay_geometry_transition_preserves_shared_binding_for_added_lane",
                "kura::tests::startup_replay_geometry_transition_rejects_checkpoint_and_manifest_drift",
                "kura::tests::startup_replay_geometry_transition_rejects_restored_lane_sidecar_drift",
                "kura::tests::startup_replay_geometry_transition_preserves_relabelled_and_retired_path_guards",
                "kura::tests::startup_replay_geometry_transition_rejects_unretained_request",
                "kura::tests::startup_replay_geometry_transition_creates_only_missing_retained_namespace_and_cleans_failure",
                "sumeragi::startup_recovery::tests::maintenance_waits_for_recovery_before_budget_or_snapshot_writes",
                "sumeragi::startup_recovery::tests::maintenance_refuses_failed_dropped_and_shutdown_recovery",
                "sumeragi::startup_recovery::tests::maintenance_retains_success_for_delayed_readonly_snapshot_subscriber",
                "sumeragi::startup_recovery::tests::snapshot_loop_stops_on_worker_failure_without_final_shutdown_write",
                "sumeragi::v2_runner::tests::authenticated_terminal_startup_idles_without_constructing_a_successor",
                "block::valid::tests::account_profile_validation_preserves_delegated_metadata_results",
                "block::valid::tests::account_profile_validation_rejects_foreign_permission_payloads",
            ),
            "network": (
                "dataspace_deploy_cli::remaining_cli_budget_keeps_original_deadline_and_never_rounds_up",
            ),
            "test-network": (
                "tests::profile_account_defaults_materialize_selected_chain_before_root_parse",
                "tests::profile_account_defaults_preserve_explicit_foreign_and_invalid_overrides",
                "tests::peer_clients_preserve_selected_network_profile_after_builder_scope",
                "tests::genesis_preexecution_preserves_selected_profile_across_threads",
                "tests::validated_genesis_cache_reuses_exact_block_and_network_identity",
                "tests::file_backed_genesis_keeps_fresh_preexecution_validation",
            ),
        }
        for scope in gate.QUALIFICATION_SCOPES:
            stages = gate.qualification_stages(scope)
            for harness, names in required.items():
                selected = [name for _, tests in stages[harness] for name in tests]
                for name in names:
                    with self.subTest(scope=scope, harness=harness, regression=name):
                        self.assertEqual(selected.count(name), 1)

    def test_both_scopes_require_exact_epoch_derivation_and_schedule_controls(self):
        required = (
            'kagemusha::derive_mint_finality_next_epoch_v1::tests::derived_parameter_matches_core_and_binds_network_epoch_and_order',
            'kagemusha::derive_mint_finality_next_epoch_v1::tests::public_context_rejects_malformed_network_epoch_count_order_and_duplicates',
            'kagemusha::derive_mint_finality_next_epoch_v1::tests::parser_exposes_only_public_arguments_and_numeric_pipe_descriptor',
            'kagemusha::derive_mint_finality_next_epoch_v1::tests::seed_reader_enforces_exact_bound_and_wipes_success_rejections_and_unwind',
            'kagemusha::derive_mint_finality_next_epoch_v1::tests::read_errors_are_redacted_and_partial_seeds_are_wiped',
            'kagemusha::derive_mint_finality_next_epoch_v1::tests::buffered_output_failures_are_returned',
            'kagemusha::derive_mint_finality_next_epoch_v1::tests::inherited_descriptor_ownership_is_closed',
            'kagemusha::derive_mint_finality_next_epoch_v1::tests::epoch_schedule_matches_native_parameters_and_preserves_exact_public_caps',
            'kagemusha::derive_mint_finality_next_epoch_v1::tests::epoch_schedule_rejects_empty_unbounded_overflowed_and_zero_fee_ranges',
            'kagemusha::derive_mint_finality_next_epoch_v1::tests::epoch_schedule_command_consumes_one_private_pipe_and_emits_only_complete_public_json',
            'kagemusha::derive_mint_finality_next_epoch_v1::tests::epoch_schedule_parser_requires_explicit_bounded_public_range_and_fee_cap',
        )
        self.assertEqual(gate.HARNESS_TARGETS["kagami"][3],
                         ["-p", "iroha_kagami", "--bin", "kagami"])
        for scope in gate.QUALIFICATION_SCOPES:
            selected = [name for _, names in gate.qualification_stages(scope)["kagami"]
                        for name in names]
            for name in required:
                with self.subTest(scope=scope, regression=name):
                    self.assertEqual(selected.count(name), 1)
                    focused = gate.focused_regression_stages(scope, ("kagami=" + name,))
                    self.assertEqual(tuple(focused), ("kagami",))
                    self.assertEqual([value for _, names in focused["kagami"]
                                      for value in names], [name])

    def test_both_scopes_require_epoch_maintenance_controls_before_network_startup(self):
        required = {
            "cli": (
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_maintenance_schedule_rejects_wrong_epoch_network_and_membership',
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_maintenance_waits_for_actual_epoch_and_preserves_carrier_deadline',
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_maintenance_preparation_binds_single_parameter_fee_and_original_lifetime',
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_maintenance_journal_preserves_one_dispatch_across_schedule_renewal',
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_maintenance_staking_preflight_rejects_fallback_and_changed_tenure',
                'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_repeat_current_preserves_freshness_and_advancing_contract',
                'taira_dataspace_deploy::finality::authenticated_height::tests::authenticated_height_restart_transport_never_masks_fixed_peer_identity',
            ),
            "network": (
                'production_beacon_bootstrap::epoch_maintenance::production_epoch_driver_admits_required_build_identity_before_setup',
                'production_beacon_bootstrap::epoch_maintenance::production_epoch_seed_pipe_rejects_shared_or_wrong_length_custody',
                'production_beacon_bootstrap::epoch_maintenance::production_epoch_schedule_requires_exact_network_roster_and_contiguous_bound',
                'production_beacon_bootstrap::canary_receipt::failed_canary_receipts_are_retained_before_parse_and_outcome_checks',
                'production_beacon_bootstrap::canary_receipt::retained_canary_receipt_requires_every_binding_and_applied_height',
            ),
        }
        real = EXPECTED_BEACON_NETWORK_TEST
        self.assertEqual([name for _, names in gate.BEACON_NETWORK_STAGES for name in names], [real])
        observations = [name for _, names in gate.NETWORK_OBSERVATION_STAGES for name in names]
        for scope in gate.QUALIFICATION_SCOPES:
            stages = gate.qualification_stages(scope)
            for harness, regressions in required.items():
                selected = [name for _, names in stages[harness] for name in names]
                for name in regressions:
                    with self.subTest(scope=scope, harness=harness, regression=name):
                        self.assertEqual(selected.count(name), 1)
                        focused = gate.focused_regression_stages(scope, (harness + "=" + name,))
                        self.assertEqual(tuple(focused), (harness,))
                        self.assertEqual([value for _, names in focused[harness] for value in names], [name])
                        if harness == "network":
                            self.assertEqual(observations.count(name), 1)
                            self.assertLess(selected.index(name), selected.index(real))

    def test_beacon_history_and_epoch_supervisor_controls_are_exact_required_and_focused(self):
        required = {
            'kagami': (
                'kura::beacon_history::tests::beacon_history_projects_only_typed_public_candidates_and_keeps_proof_limits',
                'kura::beacon_history::tests::beacon_history_distinguishes_admission_from_recorded_execution_and_nested_effects',
                'kura::beacon_history::tests::beacon_history_projects_nested_callbacks_once_and_distinguishes_rejected_roots',
                'kura::beacon_history::tests::beacon_history_requires_exact_bounded_range_and_preserves_read_only_journals',
                'kura::beacon_history::tests::beacon_history_rejects_malformed_sidecars_and_preserves_their_source',
                'kura::beacon_history::tests::beacon_history_rejects_block_height_mismatch_without_publishing_partial_json',
                'kura::beacon_history::tests::beacon_history_never_emits_opaque_install_state_or_unrelated_parameter_payloads',
                'kura::beacon_history::tests::beacon_history_cli_exposes_explicit_bounded_scope',
            ),
            'cli': (
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_maintenance_partial_initialization_never_replaces_retained_dispatch',
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_maintenance_readiness_rechecks_transition_after_completion_wait',
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_maintenance_retains_original_trust_across_explicit_release_observation',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_status_parser_has_no_seed_or_mutation_inputs',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_policy_schedule_and_custody_reject_wrong_public_authority',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_readiness_names_bind_policy_and_process_incarnation',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_rolling_batches_retain_one_epoch_overlap_and_checked_bounds',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_custody_rejects_changed_shared_and_wrong_length_seed_files',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_worker_lock_and_cursor_preserve_exclusive_restart_state',
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_supervisor_journal_guard_excludes_active_worker_until_drop',
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_supervisor_journal_guard_absence_is_read_only_and_revalidated',
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_supervisor_journal_guard_never_repairs_missing_lock',
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_supervisor_journal_guard_rejects_symlink_parent_and_child',
                'taira_dataspace_deploy::epoch_maintenance::tests::epoch_supervisor_journal_guard_rejects_parent_and_child_rebinding',
            ),
        }
        for scope in gate.QUALIFICATION_SCOPES:
            selected = gate.qualification_stages(scope)
            for harness, regressions in required.items():
                names = [name for _, tests in selected[harness] for name in tests]
                for regression in regressions:
                    with self.subTest(scope=scope, harness=harness, regression=regression):
                        self.assertEqual(names.count(regression), 1)
                        focused = gate.focused_regression_stages(scope, (harness + "=" + regression,))
                        self.assertEqual(tuple(focused), (harness,))
                        self.assertEqual([name for _, tests in focused[harness] for name in tests], [regression])
                        listing = "\n".join(name + ": test" for name in names if name != regression)
                        with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                            gate.require_tests(listing, selected[harness])

    def test_generation_reset_and_beacon_root_controls_are_exact_and_platform_required(self):
        required = {
            "core": (
                'sumeragi::v2::tests::pending_kura_standalone_apply_recovers_real_kura_shutdown_cut',
                'sumeragi::v2::tests::pending_kura_standalone_apply_rejects_foreign_owner_without_mutation',
                'sumeragi::v2::tests::pending_kura_linked_apply_recovers_real_kura_shutdown_cut',
                'sumeragi::v2::tests::pending_kura_linked_apply_rejects_changed_parent_and_decision_without_mutation',
                'sumeragi::v2::tests::pending_kura_recovered_decision_chain_recovers_real_kura_shutdown_cut',
                'sumeragi::v2::tests::pending_kura_validated_apply_preview_rejects_foreign_authority_and_fence_exhaustion_inertly',
                'sumeragi::v2::tests::production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies',
            ),
            "cli": (
                'taira_public_reset::host::epoch_supervisor::tests::epoch_public_admission_scopes_taira_and_restores_foreign_caller_profile',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_generation_admission_accepts_exact_public_inputs_without_files',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_generation_admission_rejects_foreign_origin_and_taira_profile',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_generation_admission_rejects_administrator_and_missing_genesis_grant',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_generation_admission_rejects_shared_operator_key',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_generation_admission_rejects_changed_trust_and_network',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_generation_admission_rejects_changed_custody',
                'taira_dataspace_deploy::epoch_maintenance::supervisor::tests::epoch_supervisor_generation_admission_requires_bounded_closed_schemas',
                'taira_public_reset::executor_model::tests::epoch_supervisor_pause_and_start_are_explicit_ordered_barriers',
                'taira_public_reset::executor_model::tests::epoch_supervisor_pause_failure_prevents_validator_stop',
                'taira_public_reset::executor_model::tests::maintenance_admin_admission_rejects_canary_operator_and_network_substitution',
                'taira_public_reset::executor_model::tests::old_inventory_shape_and_seven_artifact_closure_are_rejected',
                'taira_public_reset::inputs::tests::maintenance_grant_requires_registration_and_survives_no_revocation',
                'taira_public_reset::inputs::tests::ongoing_supervisor_authorization_is_explicit_and_separate_from_reset_expiry',
                'taira_public_reset::host::epoch_supervisor::tests::unit_matches_independent_python_golden_and_exact_native_argv',
                'taira_public_reset::host::epoch_supervisor::tests::first_install_pause_requires_genuine_manager_absence',
                'taira_public_reset::host::epoch_supervisor::tests::plan_rejects_wrong_administrator_origin_and_seed_role_mapping',
                'taira_public_reset::host::epoch_supervisor::tests::status_argv_contains_only_readonly_native_operation_and_exact_worker',
                'taira_public_reset::host::epoch_supervisor::tests::paths_reject_expansion_and_finite_reset_aliases',
                'taira_public_reset::host::epoch_supervisor::tests::native_status_rejects_previous_worker_or_changed_manager_incarnation',
                'taira_public_reset::host::tests::epoch_supervisor_host_frontier_has_one_pause_and_one_post_beacon_start',
                'taira_public_reset::host::epoch_seed_custody::tests::original_epoch_seed_rejects_shared_wrong_mode_length_and_symlink',
                'taira_public_reset::host::epoch_seed_custody::tests::original_epoch_seed_held_descriptor_rejects_rebinding_and_changed_content',
                'taira_public_reset::host::epoch_seed_custody::tests::original_epoch_seed_retention_is_exact_idempotent_and_never_overwrites',
                'taira_public_reset::host::epoch_seed_custody::tests::original_epoch_seed_invalid_body_does_not_create_retained_paths',
                'taira_public_reset::host::epoch_seed_custody::tests::original_epoch_seed_fifo_is_rejected_without_waiting_for_a_writer',
                'taira_public_reset::host::epoch_seed_custody::tests::original_epoch_seed_partial_staging_never_becomes_or_blocks_final',
            ),
            "kagami": (
                'kura::beacon_history::tests::beacon_history_separates_external_and_time_execution_roots_without_weakening_results',
            ),
        }
        linux_only = (
            'taira_public_reset::host::epoch_generation::linux::tests::preparation_requires_explicit_installed_and_successor_intent',
            'taira_public_reset::host::epoch_generation::linux::tests::generation_binding_rejects_alternate_cli_and_private_path',
            'taira_public_reset::host::epoch_generation::linux::tests::service_intent_never_infers_activation_from_original_absence',
        )
        affected_existing = (
            'taira_public_reset::host::tests::recovery_intent_exposes_every_ordered_child_mutation',
            'taira_public_reset::host::tests::core_testnet_scope_preserves_baseline_recovery_and_host_plan',
            'taira_public_reset::host::tests::host_receipt_names_cover_every_action_and_artifact_role',
            'taira_public_reset::host::tests::manager_evidence_stays_pending_until_exact_terminal_job',
            'taira_public_reset::host::tests::manager_evidence_accepts_captured_systemd_numeric_exit_after_deadline',
            'taira_public_reset::host::tests::manager_evidence_keeps_unexecuted_and_running_operations_pending',
            'taira_public_reset::host::tests::manager_evidence_rejects_wrong_or_duplicate_exec_identity',
        )
        for platform in ("darwin", "linux"):
            spec = importlib.util.spec_from_file_location("integration_gate", gate.__file__)
            selected_gate = importlib.util.module_from_spec(spec)
            with patch.object(sys, "platform", platform):
                spec.loader.exec_module(selected_gate)
            for scope in selected_gate.QUALIFICATION_SCOPES:
                selected = selected_gate.qualification_stages(scope)
                for harness, regressions in required.items():
                    names = [name for _, tests in selected[harness] for name in tests]
                    expected = regressions + (linux_only if harness == "cli" and platform == "linux" else ())
                    for regression in expected:
                        with self.subTest(platform=platform, scope=scope, harness=harness, regression=regression):
                            self.assertEqual(names.count(regression), 1)
                            focused = selected_gate.focused_regression_stages(scope, (harness + "=" + regression,))
                            self.assertEqual(tuple(focused), (harness,))
                            self.assertEqual([name for _, tests in focused[harness] for name in tests], [regression])
                            listing = "\n".join(name + ": test" for name in names if name != regression)
                            with self.assertRaisesRegex(selected_gate.CheckError, "required regressions missing"):
                                selected_gate.require_tests(listing, selected[harness])
                cli_names = [name for _, tests in selected["cli"] for name in tests]
                for regression in affected_existing:
                    self.assertEqual(cli_names.count(regression), 1)
                if platform == "darwin":
                    for regression in linux_only:
                        self.assertNotIn(regression, cli_names)
                        with self.assertRaisesRegex(selected_gate.CheckError, "not selected in this scope"):
                            selected_gate.focused_regression_stages(scope, ("cli=" + regression,))

    def test_public_producer_controls_replace_stale_names_and_remain_required(self):
        required = (
            "taira_dataspace_deploy::tests::journal_content_revalidation_preserves_offset_and_rejects_metadata_collisions",
            "taira_public_reset::host::epoch_seed_custody::tests::original_epoch_seed_content_binding_preserves_offset_and_rejects_metadata_collisions",
            "taira_public_reset::inputs::tests::validator_faucet_policy_requires_enabled_exact_signed_intent",
            "taira_public_reset::inputs::tests::pinned_validator_configs_reject_faucet_policy_mismatch_before_dispatch",
            'taira_public_reset::host::epoch_generation::public_binding_tests::public_projection_cannot_satisfy_native_credential_admission',
            'taira_public_reset::host::epoch_generation::public_binding_tests::public_binding_still_rejects_changed_hash_argv_and_network',
            'taira_public_reset::host::epoch_reset_inputs::tests::reset_producer_derives_exact_policy_unit_custody_and_update_binding',
            'taira_public_reset::host::epoch_reset_inputs::tests::reset_producer_rejects_implicit_prior_and_invalid_ongoing_bounds',
            'taira_public_reset::host::epoch_reset_inputs::tests::reset_producer_rejects_unmapped_sources_and_admin_genesis_substitution',
            'taira_public_reset::host::epoch_reset_inputs::tests::reset_producer_requires_explicit_until_stopped_cli_intent',
            'taira_public_reset::host::epoch_reset_inputs::tests::reset_producer_publication_is_atomic_and_never_replaces',
            'taira_public_reset::host::epoch_update_inputs::tests::epoch_update_inputs_first_install_derives_exact_native_closure_without_private_files',
            'taira_public_reset::host::epoch_update_inputs::tests::epoch_update_inputs_rejects_incomplete_or_changed_build_and_operation',
            'taira_public_reset::host::epoch_update_inputs::tests::epoch_update_inputs_preserves_original_intent_separately_from_installed_state',
            'taira_public_reset::host::epoch_update_inputs::tests::epoch_update_inputs_rejects_rebased_authority_trust_and_seed_sources',
            'taira_public_reset::host::epoch_update_inputs::tests::epoch_update_inputs_closed_preparation_has_no_implicit_state_or_receipt',
            'taira_public_reset::host::epoch_update_inputs::tests::epoch_update_inputs_output_is_atomic_private_and_never_replaced',
            'taira_public_reset::inputs::tests::topology_intent_forbids_generated_pins_and_plans',
            'taira_public_reset::inputs::tests::topology_context_checks_scope_and_budget_before_custody',
            'taira_public_reset::inputs::tests::native_context_rejects_scope_before_opening_actual_inputs',
            'taira_public_reset::public_inputs::tests::public_bundle_derives_canary_from_topology_intent_without_key_file',
            'taira_public_reset::public_inputs::tests::cli_public_input_preparation_never_accepts_private_credentials',
            'taira_public_reset::deployment_profile::tests::deployment_profile_public_context_precedes_supervisor_plan_without_weakening_export',
            'taira_public_reset::deployment_profile::tests::deployment_profile_public_context_rejects_truncated_or_extra_slot_vectors',
            'taira_public_reset::inputs::context_release::tests::reset_context_artifact_derives_real_bytes_and_retains_drift_custody',
            'taira_public_reset::inputs::context_release::tests::reset_context_artifact_rejects_wrong_mode_and_symlink_before_projection',
            'taira_public_reset::public_inputs::tests::beacon_bootstrap_window_reserves_real_queue_plan_canary_and_install',
            'taira_public_reset::public_inputs::tests::beacon_public_preparation_derives_native_nonce_bound_seats_and_rejects_substitution',
            'taira_public_reset::host::epoch_generation::completed_wrapper_tests::materialization_output_is_complete_closed_update_input',
            'taira_public_reset::host::epoch_generation::completed_wrapper_tests::completed_update_keeps_original_intent_and_actual_installed_state_distinct',
            'taira_public_reset::host::epoch_generation::completed_wrapper_tests::completed_update_rejects_foreign_receipt_and_network',
        )
        stale = (
            'taira_public_reset::inputs::tests::unsigned_inventory_draft_forbids_generated_beacon_authority',
            'taira_public_reset::public_inputs::tests::public_bundle_derives_canary_from_strict_unsigned_draft_without_key_file',
        )
        retained = (
            'tests::taira_public_reset_local_inputs_require_a_dedicated_operator_key',
            'taira_public_reset::host::epoch_supervisor::tests::epoch_public_admission_scopes_taira_and_restores_foreign_caller_profile',
        )
        for platform in ("darwin", "linux"):
            spec = importlib.util.spec_from_file_location("producer_gate", gate.__file__)
            selected_gate = importlib.util.module_from_spec(spec)
            with patch.object(sys, "platform", platform):
                spec.loader.exec_module(selected_gate)
            for scope in selected_gate.QUALIFICATION_SCOPES:
                selected = selected_gate.qualification_stages(scope)
                names = [name for _, tests in selected["cli"] for name in tests]
                for regression in required + retained:
                    with self.subTest(platform=platform, scope=scope, regression=regression):
                        self.assertEqual(names.count(regression), 1)
                        focused = selected_gate.focused_regression_stages(scope, ("cli=" + regression,))
                        self.assertEqual(tuple(focused), ("cli",))
                        self.assertEqual([name for _, tests in focused["cli"] for name in tests], [regression])
                        listing = "\n".join(name + ": test" for name in names if name != regression)
                        # A historical spelling cannot stand in for a current required test.
                        listing += "\n" + "\n".join(name + ": test" for name in stale)
                        with self.assertRaisesRegex(selected_gate.CheckError, "required regressions missing"):
                            selected_gate.require_tests(listing, selected["cli"])
                for regression in stale:
                    self.assertNotIn(regression, names)
                    with self.assertRaisesRegex(selected_gate.CheckError, "not selected in this scope"):
                        selected_gate.focused_regression_stages(scope, ("cli=" + regression,))

    def test_epoch_supervisor_platform_census_and_exact_network_fixture(self):
        preflight = 'production_beacon_bootstrap::epoch_maintenance::production_epoch_driver_admits_required_build_identity_before_setup'
        linux_identity = 'taira_public_reset::host::epoch_worker_process_identity_binds_current_kernel_incarnation'
        openssh = "taira_public_reset::host::tests::openssh_parent_pinned_inputs_survive_descriptor_sweep_without_network"
        fixtures = {
            "darwin": 'production_beacon_bootstrap::four_peer_fresh_custody_bootstrap_reaches_mandatory_pulse',
            "linux": 'production_beacon_bootstrap::epoch_maintenance::production_epoch_supervisor_renews_and_resumes_after_owned_restart',
        }
        portable_counts = (
            EXPECTED_BASIC_REGRESSION_COUNT - PLATFORM_REGRESSION_COUNT,
            EXPECTED_REGRESSION_COUNT - PLATFORM_REGRESSION_COUNT,
        )
        for platform, counts in (
            ("darwin", portable_counts),
            ("linux", tuple(count + 5 for count in portable_counts)),
        ):
            spec = importlib.util.spec_from_file_location("platform_taira_release_check", gate.__file__)
            self.assertIsNotNone(spec)
            self.assertIsNotNone(spec.loader)
            selected_gate = importlib.util.module_from_spec(spec)
            with self.subTest(platform=platform), patch.object(sys, "platform", platform):
                spec.loader.exec_module(selected_gate)
                for scope, count in zip(gate.QUALIFICATION_SCOPES, counts):
                    selected = selected_gate.qualification_stages(scope)
                    names = [name for _, tests in selected["cli"] for name in tests]
                    self.assertEqual(selected_gate.selected_regression_count(scope), count)
                    network = [name for _, tests in selected["network"] for name in tests]
                    expensive = [name for _, tests in selected_gate.BEACON_NETWORK_STAGES for name in tests]
                    self.assertEqual(expensive, [fixtures[platform]])
                    self.assertEqual(network.count(fixtures[platform]), 1)
                    other = fixtures["linux" if platform == "darwin" else "darwin"]
                    self.assertNotIn(other, network)
                    self.assertEqual(network[-1], fixtures[platform])
                    self.assertEqual(network.count(preflight), 1)
                    self.assertLess(network.index(preflight), network.index(fixtures[platform]))
                    focused_preflight = selected_gate.focused_regression_stages(scope, ("network=" + preflight,))
                    self.assertEqual([name for _, tests in focused_preflight["network"] for name in tests], [preflight])
                    without_preflight = "\n".join(name + ": test" for name in network if name != preflight)
                    with self.assertRaisesRegex(selected_gate.CheckError, "required regressions missing"):
                        selected_gate.require_tests(without_preflight, selected["network"])
                    focus = selected_gate.focused_regression_stages(scope, ("network=" + fixtures[platform],))
                    self.assertEqual([name for _, tests in focus["network"] for name in tests], expensive)
                    with self.assertRaisesRegex(selected_gate.CheckError, "not selected in this scope"):
                        selected_gate.focused_regression_stages(scope, ("network=" + other,))
                    # Retaining only the other platform's expensive case never satisfies this gate.
                    listing = "\n".join(name + ": test" for name in network if name != fixtures[platform])
                    listing += "\n" + other + ": test"
                    with self.assertRaisesRegex(selected_gate.CheckError, "required regressions missing"):
                        selected_gate.require_tests(listing, selected["network"])
                    for regression in (linux_identity, openssh):
                        self.assertEqual(names.count(regression), int(platform == "linux"))
                    if platform == "linux":
                        focused = selected_gate.focused_regression_stages(scope, ("cli=" + linux_identity,))
                        self.assertEqual([name for _, tests in focused["cli"] for name in tests], [linux_identity])
                        listing = "\n".join(name + ": test" for name in names if name != linux_identity)
                        with self.assertRaisesRegex(selected_gate.CheckError, "required regressions missing"):
                            selected_gate.require_tests(listing, selected["cli"])

    def test_basic_census_keeps_security_and_application_checks_and_defers_advanced_core(self):
        basic, full = gate.qualification_stages(), gate.qualification_stages("full")
        self.assertEqual(gate.selected_regression_count(), EXPECTED_BASIC_REGRESSION_COUNT)
        self.assertEqual(gate.selected_regression_count("full"), EXPECTED_REGRESSION_COUNT)
        self.assertEqual(set(basic), set(full))
        for name in basic:
            with self.subTest(selection=name):
                if name not in {"core", "proof-flows", "network"}:
                    self.assertEqual(basic[name], full[name])
                names = [test for _, tests in basic[name] for test in tests]
                self.assertEqual(len(names), len(set(names)))
        self.assertEqual(basic["core"], gate.CORE_ADMISSION_STARTUP_STAGES)
        for test in (
            "sumeragi::v2_effects::tests::certified_body_fence_supersession::live_idle_decision_cleanup_reconciles_runner_frontier",
            "sumeragi::v2_effects::tests::recovered_decision_fetch_fences_later_ordinary_body_coordinates",
            "sumeragi::v2_effects::tests::certified_body_fence_supersession::active_prepare_body_owners_cold_reopen_under_durable_commit",
            "sumeragi::v2_effects::tests::certified_body_fence_supersession::active_prepare_validate_cold_reopen_after_timeout_and_durable_commit",
            "sumeragi::v2_effects::tests::recovered_decision_fetch_store_publication_commits_catalogs_and_marker_together",
            "sumeragi::v2_effects::tests::recovered_decision_fetch_store_publication_rejects_partial_or_conflicting_catalogs",
            "sumeragi::v2_effects::tests::recovered_decision_fetch_store_publication_rejects_overlapping_body_stage",
            "sumeragi::v2_effects::tests::certified_body_fence_supersession::cold_decision_fetch_publishes_first_network_body_through_completion_and_apply",
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::complete_tip_decision_factory_publishes_one_authenticated_owner_open_chain",
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::complete_tip_nonempty_successor_consumes_only_the_exact_owner_open_witness",
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::owner_open_publication_chain_requires_every_exact_cas_and_is_consumed_once",
            "sumeragi::v2_core::reducer::source_link_tests::retained_body_custody_recovery_restores_work_without_voting_authority",
            "sumeragi::v2_core::reducer::source_link_tests::retained_local_body_custody_coalesces_without_downgrading_or_revalidating",
            "sumeragi::v2_core::reducer::source_link_tests::retained_body_custody_recovery_rejects_foreign_identity_and_safety_debt_atomically",
            "sumeragi::v2_core::reducer::source_link_tests::retained_body_custody_recovery_respects_the_exact_durable_decision",
            "sumeragi::v2_core::reducer::source_link_tests::retained_body_custody_preserves_normal_proposal_validation_vote_authority",
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::real_cold_owner_restores_proposal_validate_without_wal_authority",
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::real_cold_owner_coalesces_proposal_validate_with_retained_prepare_qc",
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::real_cold_owner_cancels_timeout_superseded_body_before_replay",
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::real_cold_owner_preserves_current_body_after_timeout_recovery",
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::real_cold_owner_rejects_future_body_generation_without_retirement",
            "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::frozen_ownership_peer_encoding_work_is_bounded_by_distinct_peers",
            "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::cached_peer_encodings_preserve_forged_history_and_sender_rejection",
            "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_projection_distinguishes_identical_bytes_from_distinct_origins",
            "block::valid::tests::autonomous_anchor_gas_budget_enforces_complete_source_before_anchoring",
            "sumeragi::v2_lane_work::tests::autonomous_full_block_gas_call_reserves_with_idle_catalog_route",
            "state::tests::autonomous_full_gas_sources_share_one_merge_budget_before_execution",
            "state::tests::autonomous_merge_gas_priority_preserves_old_source_and_canonical_order",
            "state::tests::autonomous_merge_gas_accounting_rejects_missing_limit_and_overflow",
            "sumeragi::v2_runner::tests::lane_evidence_repair_fence_accepts_an_empty_quarantined_replay",
            "sumeragi::v2_runner::tests::startup_reconciles_lifecycle_before_lane_work_activation",
            "sumeragi::v2_lifecycle_recovery::tests::empty_queue_reconciliation_returns_the_same_checked_receipt",
            "sumeragi::v2_lifecycle_recovery::tests::retired_nonqueue_replica_release_pending_resumes_on_startup_without_queue_owner",
            "sumeragi::v2_lifecycle_coordinator::concrete_admission::tests::terminal_signed_outputs_rejoin_after_durable_restart",
            "sumeragi::v2_runtime::tests::periodic_current_prepare_retries_bind_store_and_validate_before_lock",
            "sumeragi::v2_effects::tests::hybrid_proposal_fetch_completes_store_and_validate_with_exact_replay_root",
            "sumeragi::v2_effects::tests::proposal_fetch_store_refinement_rejects_foreign_root_and_coordinates",
            "sumeragi::v2_runtime::tests::authenticated_proposal_store_retains_root_after_fetch_or_queued_completion_upgrade",
            "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_output_cancels_same_view_proposal_after_authenticated_decision_without_timeout",
            "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_decision_proposal_cancellation_preserves_authentication_boundaries",
            "sumeragi::v2::tests::production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies",
            "sumeragi::v2::tests::production_complete_tip_activates_recovered_unapplied_decision",
            "sumeragi::v2::tests::complete_tip_decision_activation_requires_exact_replayed_wal",
            "sumeragi::v2::tests::complete_tip_decision_activation_rejects_incomplete_pending_and_applied_state",
            "sumeragi::v2::tests::complete_tip_decision_activation_preserves_exact_quorum_despite_reference_cache",
            "sumeragi::v2_core::refinement::tests::recovered_decided_successor_kernel_keeps_canonical_parent_and_commit_frontier_distinct",
            "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_proposal_cancellation_waits_for_older_ready_output",
            "sumeragi::v2_lifecycle_coordinator::open::output_recovery_tests::cold_proposal_cancellation_fsync_failure_retains_ready_owner_without_output",
            "sumeragi::v2_effects::tests::missing_replay_validate_rejects_ordinary_phase_none_binding",
            "sumeragi::v2_body_store::tests::validation_marker_publication_reuses_exact_durable_outcomes",
            "sumeragi::v2_body_store::tests::validation_marker_publication_rejects_changed_or_linked_artifacts",
            "sumeragi::v2_lifecycle_coordinator::concrete_admission::tests::terminal_timeout_certificate_reservices_only_sealed_periodic_episode",
            "smartcontracts::isi::world::isi::tests::fee_sponsor_activation_instruction_uses_requested_height_as_lower_bound",
            "smartcontracts::isi::world::isi::tests::fee_sponsor_elapsed_activation_preserves_readiness_and_authority_guards",
            "smartcontracts::isi::world::isi::tests::prospective_fee_sponsor_enrollment_funds_only_exact_self_bootstrap",
            "smartcontracts::isi::world::isi::tests::prospective_fee_sponsor_enrollment_preserves_authority_and_closed_guards",
            "state::tests::fee_sponsor_safe_activation_height_clamps_elapsed_lower_bound",
            "state::tests::fee_sponsor_safe_activation_height_preserves_later_request",
            "state::tests::fee_sponsor_safe_activation_height_fails_closed_for_non_draining_lease",
            "state::tests::fee_sponsor_revision_activation_materializes_at_scheduled_block_height",
            "state::tests::fee_sponsor_revision_activation_waits_for_old_lease_to_drain",
            "executor::tests::sponsor_resolution_predicts_scheduled_revision_only_after_old_leases_drain",
        ):
            self.assertIn(test, [test for _, tests in basic["core"] for test in tests])
        self.assertIn(
            "localnet::tests::generated_taira_genesis_grants_deployment_only_to_generated_client",
            [test for _, tests in basic["kagami"] for test in tests],
        )
        self.assertIn(
            "torii_routed_read_tests::account_permissions_handler_query_preserves_signed_pagination_and_count_mode",
            [test for _, tests in basic["torii-unit"] for test in tests],
        )
        for test in (
            "tests::account_permission_list_reads_complete_effective_fanout_before_global_pagination",
            "tests::account_permission_list_rejects_partial_or_non_effective_pages_without_output",
            "tests::account_permission_list_rejects_zero_pagination_before_http",
            "tests::account_permission_list_propagates_server_page_cap_rejection",
        ):
            self.assertIn(test, [name for _, names in basic["cli"] for name in names])
        self.assertEqual(basic["proof-flows"], ())
        self.assertTrue(full["proof-flows"])
        self.assertEqual(basic["network"], gate.BASIC_NETWORK_STAGES)
        basic_network = [
            "dataspace_deploy_cli::signed_genesis_validator_mapping_preserves_runtime_accounts",
            "dataspace_deploy_cli::phase_failure_summary_excludes_signed_payloads",
            "dataspace_deploy_cli::remaining_cli_budget_keeps_original_deadline_and_never_rounds_up",
            "runtime_catalog_transition::permission_page_tests::permission_page_requires_complete_short_fanout",
            "runtime_catalog_transition::permission_page_tests::permission_page_rejects_saturation_and_duplicate_items",
            "runtime_catalog_transition::permission_page_tests::permission_page_preserves_failure_context_and_rejects_invalid_metadata",
            "status_observation_tests::status_observation_retries_typed_busy_json_and_norito_with_remaining_budget",
            "status_observation_tests::status_observation_stops_at_original_deadline_during_retry_after",
            "status_observation_tests::status_observation_propagates_auth_other_service_and_decode_failures",
            "production_beacon_bootstrap::production_beacon_fixture_root_rejects_git_symlink_and_shared_custody",
            "production_beacon_bootstrap::production_beacon_exact_height_wait_preserves_retained_tip",
            'production_beacon_bootstrap::epoch_maintenance::production_epoch_driver_admits_required_build_identity_before_setup',
            'production_beacon_bootstrap::epoch_maintenance::production_epoch_seed_pipe_rejects_shared_or_wrong_length_custody',
            'production_beacon_bootstrap::epoch_maintenance::production_epoch_schedule_requires_exact_network_roster_and_contiguous_bound',
            'production_beacon_bootstrap::canary_receipt::failed_canary_receipts_are_retained_before_parse_and_outcome_checks',
            'production_beacon_bootstrap::canary_receipt::retained_canary_receipt_requires_every_binding_and_applied_height',
            EXPECTED_BEACON_NETWORK_TEST,
        ]
        self.assertEqual([test for _, tests in basic["network"] for test in tests], basic_network)
        self.assertEqual([test for _, tests in full["network"] for test in tests],
                         basic_network)
        for stage in gate.TORII_STARTUP_STAGES:
            self.assertIn(stage, basic["torii-unit"])
        # A promoted basic check keeps its original full-scope position.
        full_core = [name for _, names in full["core"] for name in names]
        for _, names in gate.CORE_ADMISSION_STARTUP_STAGES:
            for name in names:
                self.assertEqual(full_core.count(name), 1)
        current = "sumeragi::v2_apply::tests::ordinary_lane_frontier_preserves_third_certified_source_after_merge_execution_rejection"
        self.assertEqual(full_core.count(current), 1)
        self.assertNotIn(current, [name for _, names in basic["core"] for name in names])
        focused = gate.focused_regression_stages("full", ("core=" + current,))
        self.assertEqual(tuple(focused), ("core",))
        self.assertEqual([name for _, names in focused["core"] for name in names], [current])
        with self.assertRaisesRegex(gate.CheckError, "not selected in this scope"):
            gate.focused_regression_stages("basic", ("core=" + current,))
        listing = "\n".join(name + ": test" for name in full_core if name != current)
        with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
            gate.require_tests(listing, full["core"])
        source = SCRIPT.resolve().parents[1] / "crates/iroha_core/src/sumeragi/tests"
        leaf = (source / "v2_apply_unsealed_01c_ordinary_to_autonomous.rs").read_text()
        self.assertEqual(leaf.count("v2_apply_test!(\n    " + current.rsplit("::", 1)[1] + ","), 1)

    def test_both_scopes_require_exact_terminal_history_and_shared_outcome_recovery(self):
        required = (
            "sumeragi::v2::tests::same_round_timeout_cold_owner_preserves_retired_terminal_validation_history",
            "sumeragi::v2_body_store::tests::terminal_validate_shared_outcomes_keep_one_latest_retry_origin",
            "sumeragi::v2_body_store::tests::retired_terminal_claim_comparison_never_promotes_marker_authority",
        )
        for scope in ("basic", "full"):
            selected = [name for _, names in gate.qualification_stages(scope)["core"] for name in names]
            for name in required:
                with self.subTest(scope=scope, regression=name):
                    self.assertEqual(selected.count(name), 1)

    def test_both_scopes_execute_catalog_model_and_retained_history_regressions(self):
        required = {
            "data-model": "nexus::runtime_catalog::tests::additive_catalog_parameters_roundtrip_without_losing_canonical_identity",
            "core": "state::runtime_catalog_tests::runtime_catalog_final_overlay_rechecks_late_validator_invalidation",
            "config-unit": "parameters::actual::tests::sumeragi_v2_nexus_amx_hash_binds_committed_catalog_policy",
            "daemon": "startup_runtime_catalog_tests::startup_catalog_handoff_includes_additions_committed_during_replay",
            "network": EXPECTED_BEACON_NETWORK_TEST,
        }
        for scope in gate.QUALIFICATION_SCOPES:
            selected = gate.qualification_stages(scope)
            for harness, test in required.items():
                with self.subTest(scope=scope, harness=harness):
                    self.assertEqual([name for _, names in selected[harness] for name in names].count(test), 1)
        self.assertEqual(gate.HARNESS_TARGETS["data-model"][3], ["-p", "iroha_data_model", "--lib"])

    def test_both_scopes_require_certified_runtime_and_parameter_effects_exactly_once(self):
        required = (
            "state::tests::block_leaves_governance_unlock_audit_clean_when_no_locks_are_expired",
            "state::tests::block_sweeps_expired_governance_locks_and_records_height",
            "state::tests::block_retains_expired_governance_lock_when_atomic_release_fails",
            "state::tests::autonomous_runtime_catalog_effects_commit_and_recover_exactly",
            "state::tests::autonomous_bootstrap_parameter_effects_commit_and_recover_exactly",
            "state::tests::autonomous_runtime_catalog_effects_reject_post_stage_tampering",
            "state::tests::autonomous_parameter_effects_reject_post_stage_tampering",
            "state::tests::autonomous_runtime_catalog_effects_require_matching_pending_transition",
        )
        for scope in gate.QUALIFICATION_SCOPES:
            selected = [name for _, names in gate.qualification_stages(scope)["core"] for name in names]
            for name in required:
                with self.subTest(scope=scope, regression=name):
                    self.assertEqual(selected.count(name), 1)

    def test_both_scopes_require_retained_and_compacted_kura_replay_floor_guards(self):
        required = (
            "kura::lane_geometry::tests::configured_primary_replay_preflight_is_read_only_when_floor_is_retained",
            "kura::lane_geometry::tests::configured_primary_replay_preflight_requires_snapshot_after_compaction",
        )
        for scope in gate.QUALIFICATION_SCOPES:
            selected = [name for _, names in gate.qualification_stages(scope)["core"] for name in names]
            for name in required:
                with self.subTest(scope=scope, regression=name):
                    self.assertEqual(selected.count(name), 1)
        self.assertEqual(gate.HARNESS_TARGETS["core"][3], ["-p", "iroha_core", "--lib"])

    def test_both_scopes_require_cold_recovery_and_live_registry_authority(self):
        required = (
            "state::tests::historical_autonomous_merge_recovers_certified_carrier_before_world_replay",
            "state::tests::live_autonomous_merge_requires_exact_pending_queue_plan_owner",
            "state::tests::autonomous_merge_rejects_reforged_reservation_bindings",
            "state::tests::historical_autonomous_merge_rejects_restored_registry_conflict",
        )
        for scope in gate.QUALIFICATION_SCOPES:
            selected = [name for _, names in gate.qualification_stages(scope)["core"] for name in names]
            for name in required:
                with self.subTest(scope=scope, regression=name):
                    self.assertEqual(selected.count(name), 1)
        source_root = SCRIPT.resolve().parents[1]
        tests = source_root / "crates/iroha_core/src/state"
        self.assertIn(
            'include!("historical_merge_registry_recovery_tests.rs");',
            (tests / "autonomous_merge_and_queue_plan_tests.rs").read_text(),
        )
        leaf = (tests / "historical_merge_registry_recovery_tests.rs").read_text()
        for name in required:
            self.assertEqual(leaf.count("state_test!(consensus_stack " + name.rsplit("::", 1)[1] + "\n"), 1)

    def test_both_scopes_require_distinct_canonical_public_validator_origins(self):
        required = "taira_public_reset::executor_model::tests::validator_public_origins_require_distinct_canonical_https_roots"
        for scope in gate.QUALIFICATION_SCOPES:
            names = [name for _, tests in gate.qualification_stages(scope)["cli"] for name in tests]
            with self.subTest(scope=scope):
                self.assertEqual(names.count(required), 1)
        self.assertEqual(gate.HARNESS_TARGETS["cli"][3], ["-p", "iroha_cli", "--bin", "iroha"])

    def test_both_scopes_require_exact_native_public_input_preparation(self):
        required = (
            "taira_public_reset::public_inputs::tests::derives_native_genesis_identity_and_exact_canary_request",
            "taira_public_reset::public_inputs::tests::rejects_wrong_network_key_and_resultless_genesis",
            "taira_public_reset::public_inputs::tests::rejects_noncanonical_identity_and_non_ed25519_canary",
            "taira_public_reset::public_inputs::tests::publishes_complete_public_bundle_and_reuses_identical_request",
            "taira_public_reset::public_inputs::tests::refuses_changed_bundle_and_never_overwrites_existing_output",
            "taira_public_reset::public_inputs::tests::rejects_symlink_input_and_partial_or_surplus_output",
            "taira_public_reset::public_inputs::tests::cli_public_input_preparation_never_accepts_private_credentials",
        )
        source = SCRIPT.resolve().parents[1] / "crates/iroha_cli/src"
        self.assertIn("mod taira_public_reset;", (source / "main_shared.rs").read_text())
        self.assertIn('#[path = "taira_public_reset_public_inputs.rs"]\nmod public_inputs;',
                      (source / "taira_public_reset.rs").read_text())
        self.assertIn('#[path = "taira_public_reset_public_inputs_tests.rs"]\nmod tests;',
                      (source / "taira_public_reset_public_inputs.rs").read_text())
        leaf = (source / "taira_public_reset_public_inputs_tests.rs").read_text()
        for scope in gate.QUALIFICATION_SCOPES:
            selected = [name for _, names in gate.qualification_stages(scope)["cli"] for name in names]
            for name in required:
                with self.subTest(scope=scope, regression=name):
                    self.assertEqual(selected.count(name), 1)
                    self.assertEqual(leaf.count("fn " + name.rsplit("::", 1)[1] + "()"), 1)
        self.assertEqual(gate.HARNESS_TARGETS["cli"][3], ["-p", "iroha_cli", "--bin", "iroha"])

    def test_both_scopes_require_authenticated_execution_inclusion(self):
        required = {
            "client": (
                "client::evidence_http_tests::canonical_executed_block_reader_binds_route_wire_and_committed_evidence",
                "client::evidence_http_tests::canonical_executed_block_reader_rejects_trailing_wire_and_wrong_carrier_hash",
                "client::evidence_http_tests::canonical_executed_block_reader_requires_authenticated_execution_commitment",
            ),
            "data-model": (
                "query::canonical_output_inclusion_tests::ordinary_committed_transaction_verifies_against_exact_carrier_block",
                "query::canonical_output_inclusion_tests::authenticated_execution_inclusion_binds_complete_carrier_and_rejects_merge_authority",
                "query::canonical_output_inclusion_tests::authenticated_execution_inclusion_rejects_unbound_wire_and_header_material",
                "query::canonical_output_inclusion_tests::authenticated_execution_inclusion_joins_network_indices_without_time_inputs",
                "query::canonical_output_inclusion_tests::committed_query_rejects_retired_parallel_result_and_merge_wire",
            ),
        }
        for scope in gate.QUALIFICATION_SCOPES:
            for harness, regressions in required.items():
                selected = [name for _, names in gate.qualification_stages(scope)[harness] for name in names]
                for regression in regressions:
                    with self.subTest(scope=scope, harness=harness, regression=regression):
                        self.assertEqual(selected.count(regression), 1)
                        focused = gate.focused_regression_stages(scope, (harness + "=" + regression,))
                        self.assertEqual(tuple(focused), (harness,))
                        self.assertEqual([name for _, names in focused[harness] for name in names], [regression])
                        listing = "\n".join(name + ": test" for name in selected if name != regression)
                        with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                            gate.require_tests(listing, gate.qualification_stages(scope)[harness])
        source = SCRIPT.resolve().parents[1] / "crates/iroha_data_model/src/query"
        self.assertIn('include!("query_tail_tests.rs");', (source / "mod.rs").read_text())
        leaf = (source / "query_tail_tests.rs").read_text()
        self.assertIn('mod canonical_output_inclusion_tests {', leaf)
        for regression in required["data-model"]:
            self.assertEqual(leaf.count("    #[test]\n    fn " + regression.rsplit("::", 1)[1] + "()"), 1)
        self.assertEqual(gate.HARNESS_TARGETS["client"][3], ["-p", "iroha", "--lib"])
        self.assertEqual(gate.HARNESS_TARGETS["data-model"][3], ["-p", "iroha_data_model", "--lib"])

    def test_generated_identity_custody_controls_are_required_and_focused(self):
        required = {
            "kagami": (
                "localnet::tests::localnet_runtime_bundle_separates_ledger_and_http_operator_custody",
                "localnet::tests::generated_nexus_localnet_serves_xor_faucet_from_client_signer",
                "localnet::tests::generated_permissioned_localnet_grants_operator_exact_fee_asset_mint_permission",
                "localnet::tests::canonical_taira_generation_binds_four_runtime_signers_to_validator_peers",
            ),
            "cli": (
                "taira_public_reset::validator_config::tests::materialization_projects_split_torii_bind_without_changing_p2p_or_signer_custody",
                "taira_public_reset::validator_config::tests::materialization_rejects_invalid_torii_listener_and_port_drift",
                "taira_public_reset::validator_config::tests::materialization_torii_bind_argument_requires_canonical_ip_and_nonzero_port",
                "taira_public_reset::validator_config::tests::materialization_rejects_inheritance_identity_drift_and_source_bindings",
                "taira_public_reset::validator_config::tests::materialization_binds_every_validator_state_path_and_preserves_other_fields",
            ),
        }
        stale = "taira_public_reset::validator_config::tests::materialization_rejects_inheritance_identity_drift_and_existing_bindings"
        for platform in ("darwin", "linux"):
            spec = importlib.util.spec_from_file_location("generated_identity_gate", gate.__file__)
            selected_gate = importlib.util.module_from_spec(spec)
            with patch.object(sys, "platform", platform):
                spec.loader.exec_module(selected_gate)
            self.assertEqual(selected_gate.HARNESS_TARGETS["kagami"][3],
                             ["-p", "iroha_kagami", "--bin", "kagami"])
            for scope in selected_gate.QUALIFICATION_SCOPES:
                for harness, regressions in required.items():
                    stages = selected_gate.qualification_stages(scope)[harness]
                    names = [name for _, tests in stages for name in tests]
                    for regression in regressions:
                        with self.subTest(platform=platform, scope=scope, harness=harness, regression=regression):
                            self.assertEqual(names.count(regression), 1)
                            focused = selected_gate.focused_regression_stages(scope, (harness + "=" + regression,))
                            self.assertEqual(tuple(focused), (harness,))
                            self.assertEqual([name for _, tests in focused[harness] for name in tests], [regression])
                            listing = "\n".join(name + ": test" for name in names if name != regression)
                            listing += "\n" + stale + ": test"
                            with self.assertRaisesRegex(selected_gate.CheckError, "required regressions missing"):
                                selected_gate.require_tests(listing, stages)
                cli_names = [name for _, tests in selected_gate.qualification_stages(scope)["cli"] for name in tests]
                self.assertNotIn(stale, cli_names)
                with self.assertRaisesRegex(selected_gate.CheckError, "not selected in this scope"):
                    selected_gate.focused_regression_stages(scope, ("cli=" + stale,))

    def test_both_scopes_require_generated_genesis_and_occupied_reset_contracts(self):
        required = {
            "cli": (
                "address::tests::public_key_output_roundtrips_taira_i105_and_cli_format",
                "address::tests::public_key_output_rejects_multisig",
                "address::tests::public_key_output_rejects_malformed_and_wrong_prefix",
                "taira_public_reset::validator_config::tests::materialization_binds_every_validator_state_path_and_preserves_other_fields",
                "taira_public_reset::validator_config::tests::materialization_rejects_changed_missing_and_wrong_peer_state_paths",
                "taira_public_reset::validator_config::tests::materialization_rejects_inheritance_identity_drift_and_source_bindings",
                "taira_public_reset::validator_config::tests::materialization_requires_exact_public_genesis_identity_bytes",
                "taira_public_reset::validator_config::tests::materialization_cli_requires_explicit_custody_and_canonical_identities",
                "taira_public_reset::host::occupied::tests::occupied_runtime_rejects_builder_tools_and_each_missing_runtime_role",
                "taira_public_reset::host::epoch_supervisor::tests::prior_release_protection_preserves_independent_authenticated_tool_roots",
                "taira_public_reset::host::epoch_supervisor::tests::prior_release_protection_rejects_malformed_state_or_plan",
                "taira_public_reset::host::tests::cleanup_preserves_prior_supervisor_release_across_hosts_and_replay",
                "taira_public_reset::host::occupied::tests::occupied_runtime_accepts_split_source_and_configuration_binding",
                "taira_public_reset::host::occupied::tests::occupied_runtime_rejects_incomplete_or_foreign_artifact_custody",
                "taira_public_reset::host::occupied::tests::occupied_runtime_wire_requires_explicit_artifacts_and_argv",
                "taira_public_reset::host::occupied::tests::occupied_runtime_process_binding_distinguishes_prior_and_candidate",
                "taira_public_reset::host::occupied::tests::occupied_runtime_cleanup_preserves_every_prior_artifact_root",
                "taira_public_reset::host::occupied::tests::occupied_unit_transition_rejects_unbound_recovery_and_preserves_rollback",
                "taira_public_reset::host::occupied::tests::occupied_unit_publication_recovers_interrupted_forward_and_rollback",
                "taira_public_reset::host::occupied::tests::occupied_unit_publication_rejects_destination_or_staging_drift",
                "taira_public_reset::host::occupied::tests::occupied_unit_reload_requires_exact_terminal_manager_evidence",
                "taira_public_reset::host::occupied::tests::occupied_unit_copy_preserves_exact_mode_and_rejects_retained_mode_drift",
                "taira_public_reset::host::occupied::tests::pinned_reader_rejects_oversized_snapshot_before_allocation_and_allows_empty",
            ),
            "kagami": (
                "genesis::sign::tests::default_genesis_staging_authenticates_catalog_and_reproduces_signed_context",
                "genesis::sign::tests::public_taira_auto_bootstrap_uses_alias_bound_xor_without_config",
                "genesis::sign::tests::private_key_file_round_trips_owner_only_canonical_material",
                "genesis::sign::tests::private_key_file_rejects_unsafe_mode_links_whitespace_and_oversize",
                "localnet::tests::localnet_asset_defaults_are_selected_by_exact_taira_chain_context",
                "localnet::tests::localnet_asset_validation_rejects_selected_builtin_identity_or_alias_collision",
                "localnet::tests::canonical_taira_generation_binds_four_runtime_signers_to_validator_peers",
                "localnet::tests::generated_localnet_bootstraps_universal_kagemusha_asset",
                "localnet::tests::generated_localnet_registers_requested_asset_definition_for_client_owner",
                "localnet::tests::private_dataspace_manifests_use_the_selected_lane_alias",
            ),
        }
        for scope in gate.QUALIFICATION_SCOPES:
            stages = gate.qualification_stages(scope)
            for harness, names in required.items():
                selected = [name for _, tests in stages[harness] for name in tests]
                for name in names:
                    with self.subTest(scope=scope, harness=harness, regression=name):
                        self.assertEqual(selected.count(name), 1)
                        focused = gate.focused_regression_stages(scope, (harness + "=" + name,))
                        self.assertEqual([item for _, tests in focused[harness] for item in tests], [name])
                        listing = "\n".join(item + ": test" for item in selected if item != name)
                        with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                            gate.require_tests(listing, stages[harness])

    def test_both_scopes_require_runtime_catalog_readback_and_http_contracts_exactly_once(self):
        required = {
            "core": [
                "state::runtime_catalog_tests::runtime_catalog_readback_tracks_committed_state_and_rejects_malformed_parameter",
                "state::runtime_catalog_tests::runtime_catalog_readback_binds_next_transition_and_rejects_stale_root"
            ],
            "data-model": [
                "nexus::tests::lane_lifecycle_status_roundtrips_json_and_norito",
                "nexus::tests::lane_lifecycle_status_rejects_empty_runtime_catalog_hash",
                "nexus::tests::lane_lifecycle_status_requires_explicit_unique_nullable_runtime_catalog_hash",
                "nexus::tests::lane_lifecycle_status_rejects_forged_hash_version_and_order",
                "nexus::tests::lane_lifecycle_status_json_rejects_duplicate_unknown_and_missing_fields"
            ],
            "client": [
                "client::status_tests::lane_lifecycle_status_decodes_json_and_norito",
                "client::status_tests::lane_lifecycle_status_rejects_missing_or_empty_runtime_catalog_hash",
                "client::status_tests::lane_lifecycle_status_rejects_forged_commitment_and_malformed_payload",
                "client::status_tests::lane_lifecycle_status_requires_declared_current_media_type",
                "client::tests::get_lane_lifecycle_status_requests_typed_negotiated_snapshot"
            ],
            "torii-unit": [
                "routing::nexus_lane_lifecycle_tests::lane_lifecycle_status_binds_exact_current_catalog",
                "routing::nexus_lane_lifecycle_tests::lane_lifecycle_status_exposes_native_runtime_root_and_propagates_invalid_state",
                "openapi::tests::compact_finality_app_contracts::generated_spec_documents_read_only_nexus_lifecycle_status"
            ],
            "torii-lifecycle": [
                "nexus_lifecycle_endpoint::lifecycle_get_returns_valid_exact_json_status",
                "nexus_lifecycle_endpoint::lifecycle_get_returns_valid_exact_norito_status",
                "nexus_lifecycle_endpoint::lifecycle_get_returns_exact_present_runtime_root_in_both_formats",
                "nexus_lifecycle_endpoint::lifecycle_get_honors_api_token_access_policy",
                "nexus_lifecycle_endpoint::lifecycle_post_and_normalization_variants_are_unregistered_without_mutation"
            ]
        }
        self.assertEqual(sum(map(len, required.values())), 20)
        for scope in gate.QUALIFICATION_SCOPES:
            selected = gate.qualification_stages(scope)
            for harness, tests in required.items():
                names = [name for _, group in selected[harness] for name in group]
                for name in tests:
                    with self.subTest(scope=scope, harness=harness, regression=name):
                        self.assertEqual(names.count(name), 1)
        self.assertEqual(gate.HARNESS_TARGETS["torii-lifecycle"][3],
                         ["-p", "iroha_torii", "--test", "torii_nexus_sorafs"])

    def test_both_scopes_require_unsigned_bootstrap_and_fail_closed_capability_validation(self):
        required = {
            "client": {
                "client::tests::" + name for name in (
                    "prospective_account_submission_discovers_capabilities_without_account_auth",
                    "get_node_capabilities_json_requests_json_accept",
                    "get_node_capabilities_json_accepts_torii_utf8_json_content_type",
                    "get_node_capabilities_json_rejects_ambiguous_representation",
                    "submit_transaction_rejects_mismatched_data_model_version",
                    "submit_transaction_rejects_missing_data_model_version",
                    "submit_transaction_rejects_missing_signed_transaction_schema_hash",
                    "submit_transaction_rejects_invalid_signed_transaction_schema_hash",
                    "submit_transaction_rejects_mismatched_signed_transaction_schema_hash",
                )
            },
            "torii-unit": {
                "tests_runtime_handlers::node_capabilities_http_bootstraps_without_registered_account",
                "openapi::tests::catalog_and_contracts::account_capabilities_document_exact_public_bootstrap_policy",
                "mcp::tests::target_policy_requires_inner_canonical_proof_only_for_canonical_route",
            },
        }
        for scope in gate.QUALIFICATION_SCOPES:
            selections = gate.qualification_stages(scope)
            for harness, expected in required.items():
                with self.subTest(scope=scope, harness=harness):
                    names = [test for _, tests in selections[harness] for test in tests]
                    self.assertTrue(expected.issubset(names))
                    self.assertTrue(all(names.count(test) == 1 for test in expected))

    def test_unknown_scope_fails_before_any_source_or_build_action(self):
        for scope in ("", "skip", "core_testnet", None):
            with self.subTest(scope=scope), patch.object(gate, "shipping_harnesses") as shipping, \
                 patch.object(gate, "run_pure_fsm_checks") as fsm:
                with self.assertRaisesRegex(gate.CheckError, "scope must be basic or full"):
                    gate.run_checks(Path("/unread"), qualification_scope=scope)
                shipping.assert_not_called()
                fsm.assert_not_called()

    def test_metadata_failure_stops_codegen_fixtures_and_checkpoint_changes(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        shipping = ("cli", "kagami", "taira-launcher", "sorafs-bin")
        later = ("compile_test_harnesses", "run_config_checks", "run_stages",
                 "check_shipping_binaries", "compile_network_binaries", "run_network_checks",
                 "independent_check_evidence")
        for scope in gate.QUALIFICATION_SCOPES:
            for completed in (None, {"existing": "independent pass"}):
                with self.subTest(scope=scope, completed=completed), contextlib.ExitStack() as stack:
                    checkpoint = MagicMock()
                    stack.enter_context(patch.object(gate, "run_pure_fsm_checks"))
                    stack.enter_context(patch.object(gate, "run_lifecycle_source_checks"))
                    stack.enter_context(patch.object(gate, "shipping_harnesses", return_value=shipping))
                    downstream = [stack.enter_context(patch.object(gate, name)) for name in later]
                    stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
                    self.test_metadata.reset_mock()
                    failure = gate.CheckError("native metadata rejected macro/type error")
                    self.test_metadata.side_effect = failure
                    with self.assertRaises(gate.CheckError) as caught:
                        gate.run_checks(Path("/frozen"), qualification_scope=scope, environment=env,
                            source_commit="a" * 40, lock_fds=(77, 88),
                            completed_independent_checks=completed, update_independent_checks=checkpoint)
                    self.assertIs(caught.exception, failure)
                    self.test_metadata.assert_called_once()
                    expected = gate.native_harness_plan(gate.qualification_stages(scope), shipping)[1]
                    self.assertEqual(self.test_metadata.call_args.kwargs,
                                     {"harnesses": expected, "lock_fds": (77, 88)})
                    for action in downstream:
                        action.assert_not_called()
                    checkpoint.assert_not_called()

    def test_metadata_success_precedes_codegen_with_identical_graph_environment_and_locks(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        graphs = []
        for scope in gate.QUALIFICATION_SCOPES:
            order = []
            self.test_metadata.reset_mock()
            self.test_metadata.side_effect = lambda *_args, **_kwargs: order.append("metadata")
            def stop_at_codegen(*_args, **_kwargs):
                order.append("codegen")
                raise gate.CheckError("stop after metadata ordering assertion")
            with self.subTest(scope=scope), \
                 patch.object(gate, "run_pure_fsm_checks"), \
                 patch.object(gate, "run_lifecycle_source_checks"), \
                 patch.object(gate, "shipping_harnesses", return_value=("cli", "kagami", "taira-launcher", "sorafs-bin")), \
                 patch.object(gate, "compile_test_harnesses", side_effect=stop_at_codegen) as compile, \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "stop after metadata"):
                    gate.run_checks(Path("/frozen"), qualification_scope=scope, environment=env,
                                    source_commit="a" * 40, lock_fds=(77, 88))
            self.assertEqual(order, ["metadata", "codegen"])
            self.test_metadata.assert_called_once_with(*compile.call_args.args, **compile.call_args.kwargs)
            self.assertIs(self.test_metadata.call_args.args[1], compile.call_args.args[1])
            self.assertEqual(compile.call_args.kwargs["lock_fds"], (77, 88))
            graphs.append(compile.call_args.kwargs["harnesses"])
        self.assertEqual(graphs[0], graphs[1])

    @staticmethod
    def copies():
        copies = FixtureCopies({name: name for name in gate.HARNESS_TARGETS})
        copies.observations = [{"selection": name, "sha256": str(index) * 64, "size": 20,
                                "cargo_artifact": {"name": name}}
                               for index, name in enumerate(gate.HARNESS_TARGETS)]
        return copies

    def test_both_scopes_keep_identical_compile_graph_and_execute_exact_recorded_census(self):
        builds = []
        for scope in gate.QUALIFICATION_SCOPES:
            self.test_metadata.reset_mock()
            executed, order, output, checkpoint = [], [], io.StringIO(), MagicMock()
            copies = self.copies()
            def run(harness, root, env, stages, locks, **_kwargs):
                order.append((harness, stages))
                executed.extend((harness, test) for _, tests in stages for test in tests)
            with self.subTest(scope=scope), \
                 patch.object(gate, "shipping_harnesses", return_value=("cli", "kagami", "taira-launcher", "sorafs-bin")), \
                 patch.object(gate, "require_network_fixture_capacity"), \
                 patch.object(gate, "run_pure_fsm_checks"), \
                 patch.object(gate, "run_lifecycle_source_checks"), \
                 patch.object(gate, "compile_test_harnesses", return_value=copies) as compile, \
                 patch.object(copies, "release") as release, \
                 patch.object(gate, "run_stages", side_effect=run), \
                 patch.object(gate, "check_shipping_binaries", side_effect=lambda *args:
                              order.append(("shipping-metadata", ()))) as metadata, \
                 patch.object(gate, "run_network_checks", side_effect=lambda *args, **kwargs:
                              executed.extend(("network", test) for _, tests in kwargs["stages"] for test in tests)), \
                 contextlib.redirect_stdout(output):
                gate.run_checks(Path("/frozen"), qualification_scope=scope,
                                environment={"CARGO": "/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"},
                                source_commit="a" * 40, update_independent_checks=checkpoint)
            builds.append(compile.call_args.kwargs["harnesses"])
            self.test_metadata.assert_called_once_with(*compile.call_args.args, **compile.call_args.kwargs)
            selected = gate.qualification_stages(scope)
            expected = [(name, test) for name, stages in selected.items()
                        for _, tests in stages for test in tests]
            self.assertCountEqual(executed, expected)
            first_network = next(index for index, row in enumerate(executed) if row[0] == "network")
            for index, (harness, regression) in enumerate(executed):
                if (regression.startswith("kura::beacon_history::tests::")
                        or regression.startswith("taira_dataspace_deploy::epoch_maintenance::")
                        or regression.startswith("taira_public_reset::")):
                    self.assertLess(index, first_network, (harness, regression))
            cli_start = next(index for index, row in enumerate(executed) if row[0] == "cli")
            pending_kura = [("core", test) for _, tests in gate.CORE_PENDING_KURA_RECOVERY_STAGES
                            for test in tests]
            after_config = [row for row in executed if row[0] != "config"]
            ownership = [(name, test) for name in gate.MV_OWNERSHIP_HARNESSES
                         for _, tests in selected[name] for test in tests]
            self.assertEqual(after_config[:len(ownership) + len(pending_kura)],
                             ownership + pending_kura)
            startup = {"core": gate.CORE_STARTUP_STAGES, "torii-unit": gate.TORII_STARTUP_STAGES,
                       "daemon": gate.DAEMON_STARTUP_STAGES}
            for name, stages in startup.items():
                for _, tests in stages:
                    for test in tests:
                        if (name, test) in expected:
                            self.assertLess(executed.index((name, test)), cli_start)
            metadata.assert_called_once()
            shipping_index = order.index(("shipping-metadata", ()))
            self.assertLess(shipping_index, next(index for index, row in enumerate(order) if row[0] == "cli"))
            self.assertEqual(order[0], ("config", gate.CONFIG_STAGES))
            for index, (name, stages) in enumerate(order):
                if name in gate.MV_OWNERSHIP_HARNESSES or stages == gate.CORE_PENDING_KURA_RECOVERY_STAGES or (
                        name in startup and stages and all(stage in startup[name] for stage in stages)):
                    self.assertLess(index, shipping_index)
            self.assertEqual(len(executed), gate.selected_regression_count(scope))
            self.assertEqual(checkpoint.call_args_list[0].args, (None,))
            evidence = checkpoint.call_args_list[1].args[0]
            self.assertEqual(evidence["qualification_scope"], scope)
            recorded = [(row["selection"], test) for row in evidence["selected_tests"]
                        for stage in row["stages"] for test in stage["tests"]]
            self.assertCountEqual(recorded, [row for row in executed if row[0] not in {"config", "network"}])
            self.assertIn(f"PASS: {len(expected)} {scope} regressions", output.getvalue())
            if scope == "basic":
                self.assertIn("proof-flows", builds[-1])
                self.assertNotIn("proof-flows", {row["selection"] for row in evidence["artifacts"]})
                self.assertIn(unittest.mock.call("proof-flows"), release.call_args_list)
        self.assertEqual(builds[0], builds[1])

    def test_independent_evidence_cannot_cross_scope_even_with_identical_artifacts_and_cases(self):
        stages = (("cli", gate.STAGES),)
        basic = gate.independent_check_evidence(self.copies(), stages, qualification_scope="basic")
        full = gate.independent_check_evidence(self.copies(), stages, qualification_scope="full")
        self.assertNotEqual(basic, full)
        self.assertEqual(basic | {"qualification_scope": "full"}, full)

    def test_startup_failures_are_collected_before_cli_other_groups_or_network(self):
        executed = []
        def fail_startup(harness, root, env, stages, locks, **_kwargs):
            executed.append(harness)
            if harness in {"core", "daemon"} and stages != gate.CORE_PENDING_KURA_RECOVERY_STAGES:
                raise gate.SelectedRegressionFailures([harness + " startup failed"])
        for scope in gate.QUALIFICATION_SCOPES:
            executed.clear()
            checkpoint, output = MagicMock(), io.StringIO()
            with self.subTest(scope=scope), \
                 patch.object(gate, "shipping_harnesses", return_value=("kagami",)), \
                 patch.object(gate, "require_network_fixture_capacity"), \
                 patch.object(gate, "run_pure_fsm_checks"), \
                 patch.object(gate, "run_lifecycle_source_checks"), \
                 patch.object(gate, "compile_test_harnesses", return_value=self.copies()), \
                 patch.object(gate, "run_stages", side_effect=fail_startup), \
                 patch.object(gate, "check_shipping_binaries") as metadata, \
                 patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(output):
                with self.assertRaises(gate.SelectedRegressionFailures) as failure:
                    gate.run_checks(Path("/frozen"), qualification_scope=scope,
                                    environment={"CARGO": "/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"},
                                    source_commit="a" * 40, update_independent_checks=checkpoint)
            self.assertEqual(failure.exception.failures, ("core startup failed", "daemon startup failed"))
            self.assertEqual(executed, ["config", "mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread", "core", "core", "torii-unit", "daemon"])
            checkpoint.assert_called_once_with(None)
            metadata.assert_not_called()
            network.assert_not_called()
            self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_pending_kura_failure_stops_before_other_startup_shipping_or_network(self):
        for scope in gate.QUALIFICATION_SCOPES:
            for error in (gate.SelectedRegressionFailures(["pending Apply publication failed"]),
                          gate.CheckError("required regressions missing from native harness")):
                executed, order, output, checkpoint = [], [], io.StringIO(), MagicMock()
                copies = self.copies()
                def compile_batch(*args, **kwargs):
                    order.append("compiled")
                    return copies
                def run(harness, root, env, stages, locks, **_kwargs):
                    order.append(harness)
                    executed.append((harness, stages))
                    if harness == "core":
                        self.assertEqual(stages, gate.CORE_PENDING_KURA_RECOVERY_STAGES)
                        raise error
                with self.subTest(scope=scope, error=type(error).__name__), \
                     patch.object(gate, "shipping_harnesses", return_value=("kagami",)), \
                     patch.object(gate, "require_network_fixture_capacity"), \
                     patch.object(gate, "run_pure_fsm_checks"), \
                     patch.object(gate, "run_lifecycle_source_checks"), \
                     patch.object(gate, "compile_test_harnesses", side_effect=compile_batch) as compile, \
                     patch.object(gate, "run_stages", side_effect=run), \
                     patch.object(gate, "check_shipping_binaries") as metadata, \
                     patch.object(gate, "compile_network_binaries") as shipping, \
                     patch.object(gate, "run_network_checks") as network, \
                     contextlib.redirect_stdout(output):
                    with self.assertRaises(type(error)) as failure:
                        gate.run_checks(Path("/frozen"), qualification_scope=scope,
                                        environment={"CARGO": "/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"},
                                        source_commit="a" * 40, update_independent_checks=checkpoint)
                self.assertIs(failure.exception, error)
                self.assertEqual(order, ["compiled", "config", "mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread", "core"])
                self.assertEqual(executed, [("config", gate.CONFIG_STAGES),
                                            ("mv", gate.MV_OWNERSHIP_STAGES),
                                            ("mv-ebr", gate.MV_EBR_STAGES),
                                            ("mv-map", gate.MV_MAP_STAGES),
                                            ("mv-admitted-map", gate.MV_ADMITTED_MAP_STAGES),
                                            ("concread", gate.CONCREAD_STAGES),
                                            ("core", gate.CORE_PENDING_KURA_RECOVERY_STAGES)])
                compile.assert_called_once()
                checkpoint.assert_called_once_with(None)
                metadata.assert_not_called()
                shipping.assert_not_called()
                network.assert_not_called()
                self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_mv_failure_stops_before_core_shipping_network_and_checkpoint_success(self):
        for scope in gate.QUALIFICATION_SCOPES:
            for failed in gate.MV_OWNERSHIP_HARNESSES:
                for error in (gate.SelectedRegressionFailures([failed + " ownership failed"]),
                              gate.CheckError("required regressions missing from native harness")):
                    executed, output, checkpoint = [], io.StringIO(), MagicMock()
                    copies = self.copies()

                    def run(harness, root, env, stages, locks, **_kwargs):
                        executed.append((harness, stages))
                        if harness == failed:
                            raise error

                    with self.subTest(scope=scope, failed=failed, error=type(error).__name__), \
                         patch.object(gate, "shipping_harnesses", return_value=()), \
                         patch.object(gate, "require_network_fixture_capacity"), \
                         patch.object(gate, "run_pure_fsm_checks"), \
                         patch.object(gate, "run_lifecycle_source_checks"), \
                         patch.object(gate, "compile_test_harnesses", return_value=copies) as compile, \
                         patch.object(FixtureCopies, "__exit__", return_value=False) as close, \
                         patch.object(gate, "run_stages", side_effect=run), \
                         patch.object(gate, "compile_network_binaries") as shipping, \
                         patch.object(gate, "run_network_checks") as network, \
                         contextlib.redirect_stdout(output):
                        with self.assertRaises(type(error)) as failure:
                            gate.run_checks(Path("/frozen"), qualification_scope=scope,
                                            environment={"CARGO": "/cargo", "CARGO_HOME": "/isolated",
                                                         "CARGO_TARGET_DIR": "/warm"},
                                            source_commit="a" * 40,
                                            update_independent_checks=checkpoint)
                    selected = gate.qualification_stages(scope)
                    expected = ("config", *gate.MV_OWNERSHIP_HARNESSES[
                        :gate.MV_OWNERSHIP_HARNESSES.index(failed) + 1])
                    self.assertEqual(executed, [(name, selected[name]) for name in expected])
                    self.assertIs(failure.exception, error)
                    compile.assert_called_once()
                    close.assert_called_once()
                    self.assertIs(close.call_args.args[1], error)
                    checkpoint.assert_called_once_with(None)
                    shipping.assert_not_called()
                    network.assert_not_called()
                    self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_changed_mv_artifact_or_census_invalidates_independent_checkpoint(self):
        for scope in gate.QUALIFICATION_SCOPES:
            for harness in gate.MV_OWNERSHIP_HARNESSES:
                for changed in ("artifact", "census"):
                    copies, checkpoint = self.copies(), MagicMock()
                    selected = gate.qualification_stages(scope)
                    early, _, _ = gate.native_harness_plan(selected, ())
                    evidence = gate.independent_check_evidence(
                        copies, (("cli", gate.STAGES),) + early, qualification_scope=scope)
                    evidence = json.loads(json.dumps(evidence))
                    if changed == "artifact":
                        row = next(row for row in evidence["artifacts"] if row["selection"] == harness)
                        row["sha256"] = "changed-mv-artifact"
                    else:
                        row = next(row for row in evidence["selected_tests"] if row["selection"] == harness)
                        row["stages"][0]["tests"].pop()
                    error = gate.SelectedRegressionFailures(["fresh MV preflight failed"])

                    def run(name, *args, **_kwargs):
                        if name == "mv":
                            raise error

                    with self.subTest(scope=scope, harness=harness, changed=changed), \
                         patch.object(gate, "shipping_harnesses", return_value=()), \
                         patch.object(gate, "require_network_fixture_capacity"), \
                         patch.object(gate, "run_pure_fsm_checks"), \
                         patch.object(gate, "run_lifecycle_source_checks"), \
                         patch.object(gate, "compile_test_harnesses", return_value=copies), \
                         patch.object(gate, "run_stages", side_effect=run) as execute, \
                         patch.object(gate, "run_network_checks") as network, \
                         contextlib.redirect_stdout(io.StringIO()):
                        with self.assertRaises(gate.SelectedRegressionFailures) as failure:
                            gate.run_checks(Path("/frozen"), qualification_scope=scope,
                                            environment={"CARGO": "/cargo", "CARGO_HOME": "/isolated",
                                                         "CARGO_TARGET_DIR": "/warm"},
                                            source_commit="a" * 40,
                                            completed_independent_checks=evidence,
                                            update_independent_checks=checkpoint)
                    self.assertIs(failure.exception, error)
                    self.assertEqual([call.args[0] for call in execute.call_args_list], ["config", "mv"])
                    checkpoint.assert_called_once_with(None)
                    network.assert_not_called()

    def test_exact_independent_checkpoint_reuses_startup_and_cli_passes_in_both_scopes(self):
        for scope in gate.QUALIFICATION_SCOPES:
            copies, checkpoint = self.copies(), MagicMock()
            selected = gate.qualification_stages(scope)
            shipping = ("taira-launcher", "cli", "sorafs-bin", "kagami")
            early, _, _ = gate.native_harness_plan(selected, shipping)
            independent = (("cli", selected["cli"]),) + early
            evidence = gate.independent_check_evidence(copies, independent, qualification_scope=scope)
            with self.subTest(scope=scope), \
                 patch.object(gate, "shipping_harnesses", return_value=shipping), \
                 patch.object(gate, "require_network_fixture_capacity"), \
                 patch.object(gate, "run_pure_fsm_checks"), \
                 patch.object(gate, "run_lifecycle_source_checks"), \
                 patch.object(gate, "compile_test_harnesses", return_value=copies) as compile, \
                 patch.object(gate, "run_stages") as run, \
                 patch.object(gate, "check_shipping_binaries") as metadata, \
                 patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(io.StringIO()):
                gate.run_checks(Path("/frozen"), qualification_scope=scope,
                                environment={"CARGO": "/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"},
                                source_commit="a" * 40, completed_independent_checks=evidence,
                                update_independent_checks=checkpoint)
            compile.assert_called_once()
            run.assert_called_once()
            self.assertEqual(run.call_args.args[3], gate.CONFIG_STAGES)
            metadata.assert_called_once()
            network.assert_called_once()
            checkpoint.assert_not_called()

    def test_shipping_metadata_failure_stops_long_tests_network_and_new_checkpoint(self):
        shipping = ("taira-launcher", "cli", "sorafs-bin", "kagami")
        for scope in gate.QUALIFICATION_SCOPES:
            for reuse in (False, True):
                copies, checkpoint, output, executed = self.copies(), MagicMock(), io.StringIO(), []
                selected = gate.qualification_stages(scope)
                early, _, _ = gate.native_harness_plan(selected, shipping)
                evidence = gate.independent_check_evidence(
                    copies, (("cli", selected["cli"]),) + early, qualification_scope=scope)
                failure = gate.CheckError("shipping metadata check failed (exit 101)")
                with self.subTest(scope=scope, reuse=reuse), \
                     patch.object(gate, "shipping_harnesses", return_value=shipping), \
                     patch.object(gate, "run_pure_fsm_checks"), \
                     patch.object(gate, "run_lifecycle_source_checks"), \
                     patch.object(gate, "compile_test_harnesses", return_value=copies), \
                     patch.object(gate, "run_stages", side_effect=lambda name, root, env, stages, locks, **kwargs:
                                  executed.append((name, stages))), \
                     patch.object(gate, "check_shipping_binaries", side_effect=failure) as metadata, \
                     patch.object(gate, "run_network_checks") as network, \
                     patch.object(gate, "compile_network_binaries") as codegen, \
                     contextlib.redirect_stdout(output):
                    with self.assertRaises(gate.CheckError) as error:
                        gate.run_checks(Path("/frozen"), qualification_scope=scope,
                                        environment={"CARGO": "/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"},
                                        source_commit="a" * 40, lock_fds=(77, 88),
                                        completed_independent_checks=evidence if reuse else None,
                                        update_independent_checks=checkpoint)
                self.assertIs(error.exception, failure)
                metadata.assert_called_once()
                self.assertEqual(metadata.call_args.args[0], Path("/frozen"))
                self.assertEqual(metadata.call_args.args[1]["CARGO_TARGET_DIR"], "/warm")
                self.assertEqual(metadata.call_args.args[2], (77, 88))
                self.assertNotIn("cli", [name for name, _ in executed])
                allowed = {"config": gate.CONFIG_STAGES, "core": gate.CORE_STARTUP_STAGES,
                           "torii-unit": gate.TORII_STARTUP_STAGES, "daemon": gate.DAEMON_STARTUP_STAGES,
                           **{name: selected[name] for name in gate.MV_OWNERSHIP_HARNESSES}}
                for name, stages in executed:
                    self.assertTrue(all(stage in allowed[name] for stage in stages))
                if reuse:
                    self.assertEqual(executed, [("config", gate.CONFIG_STAGES)])
                    checkpoint.assert_not_called()
                else:
                    checkpoint.assert_called_once_with(None)
                network.assert_not_called()
                codegen.assert_not_called()
                self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_standalone_cli_selects_basic_by_default_and_forwards_explicit_full(self):
        import taira_release as release
        for arguments, scope in (([], "basic"), (["--native-check-scope", "full"], "full")):
            with self.subTest(scope=scope), patch.object(sys, "argv", [str(SCRIPT), *arguments]), \
                 patch.object(release, "development_check") as check:
                self.assertEqual(gate.main(), 0)
                self.assertEqual(check.call_args.kwargs, {"native_check_scope": scope, "native_linker": "llvm" if sys.platform == "linux" else "system"})


class FocusedPrequalificationTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)
        self.env = {"CARGO": "/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        self.core = "state::tests::historical_autonomous_merge_recovers_certified_carrier_before_world_replay"
        self.cli = next(name for _, names in gate.STAGES for name in names)
        self.network = EXPECTED_BEACON_NETWORK_TEST
        for name in ("run_pure_fsm_checks", "run_lifecycle_source_checks", "require_network_fixture_capacity"):
            mock = patch.object(gate, name)
            mock.start()
            self.addCleanup(mock.stop)
        git = patch.object(gate.subprocess, "check_output", return_value="a" * 40 + "\n")
        self.git = git.start()
        self.addCleanup(git.stop)
        metadata = patch.object(gate, "check_test_harnesses")
        self.metadata = metadata.start()
        self.addCleanup(metadata.stop)

    def test_mv_only_focus_builds_explicit_ownership_targets_without_qualification(self):
        for scope in gate.QUALIFICATION_SCOPES:
            selected = gate.qualification_stages(scope)
            requested = tuple(harness + "=" + test for harness in gate.MV_OWNERSHIP_HARNESSES
                              for _, tests in selected[harness] for test in tests)
            self.assertEqual(len(requested), 94)
            copies = FixtureCopies({name: "/copies/" + name for name in gate.HARNESS_TARGETS})
            output = io.StringIO()
            with self.subTest(scope=scope), \
                 patch.object(gate, "compile_test_harnesses", return_value=copies) as compile, \
                 patch.object(gate, "run_stages") as run, \
                 patch.object(gate, "compile_network_binaries") as shipping, \
                 patch.object(gate, "run_network_checks") as network, \
                 patch.object(gate, "independent_check_evidence") as evidence, \
                 contextlib.redirect_stdout(output):
                gate.run_prequalification(Path("/mutable"), qualification_scope=scope,
                    focused_regressions=requested, environment=self.env, lock_fds=(91,))
            compile.assert_called_once()
            self.metadata.assert_called_once_with(*compile.call_args.args, **compile.call_args.kwargs)
            self.metadata.reset_mock()
            self.assertEqual(compile.call_args.kwargs["harnesses"], ("config", "mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread"))
            self.assertEqual(compile.call_args.kwargs["lock_fds"], (91,))
            self.assertEqual([call.args[0] for call in run.call_args_list],
                             ["/copies/" + name for name in ("config", "mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread")])
            self.assertEqual([call.args[3] for call in run.call_args_list],
                             [gate.CONFIG_STAGES, *[selected[name] for name in gate.MV_OWNERSHIP_HARNESSES]])
            shipping.assert_not_called()
            network.assert_not_called()
            evidence.assert_not_called()
            self.assertIn("94 focused regressions", output.getvalue())
            self.assertIn("NOT release qualification", output.getvalue())
            self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_focus_requires_exact_distinct_current_scope_selections_before_tools(self):
        for requested in (None, (), "core=" + self.core, ("",), ("core=*",),
                          ("unknown=" + self.core,), ("core=" + self.core,) * 2,
                          ("network=four_peer_multiroute_public_transaction_sequence_reaches_applied",)):
            with self.subTest(requested=requested), patch.object(gate, "compile_test_harnesses") as compile:
                with self.assertRaises(gate.CheckError):
                    gate.run_prequalification(Path("/mutable"), focused_regressions=requested,
                                              environment=self.env, lock_fds=())
                compile.assert_not_called()
        self.git.assert_not_called()
        self.metadata.assert_not_called()
        focused = gate.focused_regression_stages("basic", ("core=" + self.core,))
        self.assertEqual([name for _, names in focused["core"] for name in names], [self.core])

    def test_prequalification_checks_only_requested_harnesses_while_qualification_keeps_complete_graph(self):
        for scope in gate.QUALIFICATION_SCOPES:
            copies = FixtureCopies({name: "/copies/" + name for name in gate.HARNESS_TARGETS})
            output = io.StringIO()
            with self.subTest(scope=scope), \
                 patch.object(gate, "compile_test_harnesses", return_value=copies) as compile, \
                 patch.object(gate, "run_stages") as run, \
                 patch.object(gate, "run_network_checks") as network, \
                 contextlib.redirect_stdout(output):
                gate.run_checks(Path("/mutable"), qualification_scope=scope,
                                environment=self.env, source_commit="a" * 40, lock_fds=(91,))
                complete_graph = compile.call_args.kwargs["harnesses"]
                self.metadata.assert_called_once_with(*compile.call_args.args, **compile.call_args.kwargs)
                self.metadata.reset_mock()
                compile.reset_mock(); run.reset_mock(); network.reset_mock()
                output.seek(0); output.truncate(0)
                gate.run_prequalification(Path("/mutable"), qualification_scope=scope,
                    focused_regressions=("core=" + self.core,), environment=self.env, lock_fds=(91,))
            compile.assert_called_once()
            self.metadata.assert_called_once_with(*compile.call_args.args, **compile.call_args.kwargs)
            self.metadata.reset_mock()
            self.assertEqual(compile.call_args.kwargs["harnesses"], ("config", "core"))
            self.assertEqual(compile.call_args.kwargs["lock_fds"], (91,))
            self.assertIn("network", complete_graph, "immutable qualification still compiles network")
            self.assertIn("proof-flows", complete_graph, "immutable qualification keeps its full graph")
            self.assertEqual([call.args[0] for call in run.call_args_list], ["/copies/config", "/copies/core"])
            self.assertEqual(run.call_args_list[0].args[3], gate.CONFIG_STAGES)
            self.assertEqual([name for _, names in run.call_args_list[1].args[3] for name in names], [self.core])
            network.assert_not_called()
            self.assertIn("NOT release qualification", output.getvalue())
            self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_configuration_failure_stops_focused_execution_and_network(self):
        copies = FixtureCopies({name: "/copies/" + name for name in gate.HARNESS_TARGETS})
        with patch.object(gate, "compile_test_harnesses", return_value=copies), \
             patch.object(gate, "run_stages", side_effect=gate.SelectedRegressionFailures(["config failed"])) as run, \
             patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(gate.SelectedRegressionFailures):
                gate.run_prequalification(Path("/mutable"),
                    focused_regressions=("core=" + self.core, "network=" + self.network),
                    environment=self.env, lock_fds=())
        run.assert_called_once()
        self.assertEqual(run.call_args.args[3], gate.CONFIG_STAGES)
        network.assert_not_called()

    def test_selected_pending_kura_runs_first_once_without_expanding_focus(self):
        recovery = tuple(test for _, tests in gate.CORE_PENDING_KURA_RECOVERY_STAGES for test in tests)
        beacon = gate.CORE_BEACON_STAGES[0][1][0]
        daemon = gate.DAEMON_STARTUP_STAGES[0][1][0]
        for scope in gate.QUALIFICATION_SCOPES:
            for selected in (recovery, (recovery[1], recovery[-1])):
                copies = FixtureCopies({name: name for name in gate.HARNESS_TARGETS})
                executed, output = [], io.StringIO()
                requested = ("core=" + beacon, "daemon=" + daemon, "cli=" + self.cli,
                             "network=" + self.network) + tuple("core=" + test for test in reversed(selected))
                def execute(harness, root, env, stages, locks, **kwargs):
                    self.assertNotIn(unittest.mock.call(harness), release.call_args_list)
                    self.assertEqual(locks, (91,))
                    executed.extend((harness, test) for _, tests in stages for test in tests)
                with self.subTest(scope=scope, selected=selected), \
                     patch.object(gate, "compile_test_harnesses", return_value=copies), \
                     patch.object(copies, "release") as release, \
                     patch.object(gate, "run_stages", side_effect=execute) as run, \
                     patch.object(gate, "run_network_checks", side_effect=lambda *args, **kwargs:
                                  executed.extend(("network", test) for _, tests in kwargs["stages"]
                                                  for test in tests)), \
                     contextlib.redirect_stdout(output):
                    gate.run_prequalification(Path("/mutable"), qualification_scope=scope,
                        focused_regressions=requested, environment=self.env, lock_fds=(91,))
                config = [("config", test) for _, tests in gate.CONFIG_STAGES for test in tests]
                expected = config + [tuple(item.split("=", 1)) for item in requested]
                self.assertCountEqual(executed, expected)
                self.assertEqual(executed[:len(config) + len(selected)],
                                 config + [("core", test) for test in selected])
                self.assertEqual([call.args[0] for call in run.call_args_list],
                                 ["config", "core", "core", "daemon", "cli"])
                self.assertEqual(run.call_args_list[-1].kwargs, {"batch": True})
                self.assertEqual(release.call_args_list.count(unittest.mock.call("core")), 1)
                self.assertIn(f"{len(requested)} focused regressions", output.getvalue())

    def test_pending_kura_only_focus_releases_core_without_empty_or_duplicate_run(self):
        selected = gate.CORE_PENDING_KURA_RECOVERY_STAGES[0][1][-1]
        copies = FixtureCopies({name: name for name in gate.HARNESS_TARGETS})
        with patch.object(gate, "compile_test_harnesses", return_value=copies), \
             patch.object(copies, "release") as release, \
             patch.object(gate, "run_stages") as run, \
             patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(io.StringIO()):
            gate.run_prequalification(Path("/mutable"), focused_regressions=("core=" + selected,),
                                      environment=self.env, lock_fds=())
        self.assertEqual([call.args[0] for call in run.call_args_list], ["config", "core"])
        self.assertEqual([test for _, tests in run.call_args_list[1].args[3] for test in tests], [selected])
        self.assertEqual(release.call_args_list, [unittest.mock.call("config"), unittest.mock.call("core")])
        network.assert_not_called()

    def test_pending_kura_failure_stops_all_remaining_focused_execution(self):
        selected = gate.CORE_PENDING_KURA_RECOVERY_STAGES[0][1][2]
        for scope in gate.QUALIFICATION_SCOPES:
            for error in (gate.SelectedRegressionFailures(["pending Apply publication failed"]),
                          gate.CheckError("required regressions missing from native harness")):
                copies = FixtureCopies({name: name for name in gate.HARNESS_TARGETS})
                output = io.StringIO()
                def execute(harness, root, env, stages, locks, **kwargs):
                    if harness == "core":
                        self.assertEqual([test for _, tests in stages for test in tests], [selected])
                        raise error
                with self.subTest(scope=scope, error=type(error).__name__), \
                     patch.object(gate, "compile_test_harnesses", return_value=copies), \
                     patch.object(gate, "run_stages", side_effect=execute) as run, \
                     patch.object(gate, "run_network_checks") as network, \
                     patch.object(gate, "compile_network_binaries") as shipping, \
                     contextlib.redirect_stdout(output):
                    with self.assertRaises(type(error)) as failed:
                        gate.run_prequalification(Path("/mutable"), qualification_scope=scope,
                            focused_regressions=("core=" + self.core, "core=" + selected,
                                                 "cli=" + self.cli, "network=" + self.network),
                            environment=self.env, lock_fds=())
                self.assertIs(failed.exception, error)
                self.assertEqual([call.args[0] for call in run.call_args_list], ["config", "core"])
                network.assert_not_called()
                shipping.assert_not_called()
                self.assertNotIn("diagnostic passed", output.getvalue())

    def test_independent_focused_failures_aggregate_and_prevent_network(self):
        copies = FixtureCopies({name: "/copies/" + name for name in gate.HARNESS_TARGETS})
        selected = gate.CORE_PENDING_KURA_RECOVERY_STAGES[0][1][0]
        def execute(harness, root, env, stages, locks, **_kwargs):
            names = [test for _, tests in stages for test in tests]
            if harness != "/copies/config" and names != [selected]:
                raise gate.SelectedRegressionFailures([harness + " failed"])
        with patch.object(gate, "compile_test_harnesses", return_value=copies) as compile, \
             patch.object(gate, "run_stages", side_effect=execute) as run, \
             patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(gate.SelectedRegressionFailures) as failed:
                gate.run_prequalification(Path("/mutable"),
                    focused_regressions=("core=" + self.core, "core=" + selected,
                                         "cli=" + self.cli, "network=" + self.network),
                    environment=self.env, lock_fds=())
        self.assertEqual([call.args[0] for call in run.call_args_list],
                         ["/copies/config", "/copies/core", "/copies/core", "/copies/cli"])
        self.assertEqual(compile.call_args.kwargs["harnesses"], ("config", "core", "network", "cli"))
        self.assertEqual(failed.exception.failures, ("/copies/core failed", "/copies/cli failed"))
        self.assertEqual([call.kwargs for call in run.call_args_list], [{}, {}, {}, {"batch": True}])
        network.assert_not_called()

    def test_configuration_focus_is_not_duplicated_and_network_receives_exact_focus(self):
        config = next(name for _, names in gate.CONFIG_STAGES for name in names)
        copies = FixtureCopies({name: "/copies/" + name for name in gate.HARNESS_TARGETS})
        with patch.object(gate, "compile_test_harnesses", return_value=copies), \
             patch.object(gate, "run_stages") as run, \
             patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(io.StringIO()):
            gate.run_prequalification(Path("/mutable"), focused_regressions=(
                "config=" + config, "network=" + self.network), environment=self.env, lock_fds=(92,))
        run.assert_called_once()
        self.assertEqual(run.call_args.args[3], gate.CONFIG_STAGES)
        network.assert_called_once()
        self.assertEqual([name for _, names in network.call_args.kwargs["stages"] for name in names], [self.network])
        self.assertEqual(network.call_args.args[3], (92,))

    def test_prequalification_has_no_signed_source_or_checkpoint_interface(self):
        for forbidden in ("source_commit", "completed_independent_checks", "update_independent_checks"):
            with self.subTest(forbidden=forbidden), patch.object(gate, "compile_test_harnesses") as compile:
                with self.assertRaises(TypeError):
                    gate.run_prequalification(Path("/mutable"), focused_regressions=("core=" + self.core,),
                        environment=self.env, lock_fds=(), **{forbidden: "not accepted"})
                compile.assert_not_called()

    def test_metadata_failure_stops_before_codegen_config_and_network(self):
        self.metadata.side_effect = gate.CheckError("native test metadata check failed: E0432")
        output = io.StringIO()
        with patch.object(gate, "compile_test_harnesses") as compile, \
             patch.object(gate, "run_config_checks") as config, \
             patch.object(gate, "run_stages") as run, \
             patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(output):
            with self.assertRaisesRegex(gate.CheckError, "E0432"):
                gate.run_prequalification(Path("/mutable"), focused_regressions=(
                    "network=" + self.network,), environment=self.env, lock_fds=(91,))
        self.metadata.assert_called_once()
        compile.assert_not_called()
        config.assert_not_called()
        run.assert_not_called()
        network.assert_not_called()
        self.assertNotIn("diagnostic passed", output.getvalue())

    def test_metadata_success_still_requires_full_codegen_before_regressions(self):
        order = []
        self.metadata.side_effect = lambda *args, **kwargs: order.append("metadata")
        def build(*args, **kwargs):
            order.append("build")
            raise gate.CheckError("codegen-only failure")
        with patch.object(gate, "compile_test_harnesses", side_effect=build) as compile, \
             patch.object(gate, "run_config_checks") as config, \
             patch.object(gate, "run_stages") as run, \
             patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "codegen-only failure"):
                gate.run_prequalification(Path("/mutable"), focused_regressions=(
                    "core=" + self.core,), environment=self.env, lock_fds=(91,))
        self.assertEqual(order, ["metadata", "build"])
        self.assertEqual(self.metadata.call_args, compile.call_args)
        config.assert_not_called()
        run.assert_not_called()
        network.assert_not_called()

    def test_standalone_focus_cli_forwards_the_explicit_diagnostic(self):
        import taira_release as release
        with patch.object(sys, "argv", [str(SCRIPT), "--focus-regression", "core=" + self.core]), \
             patch.object(release, "development_check") as check:
            self.assertEqual(gate.main(), 0)
        self.assertEqual(check.call_args.kwargs, {
            "native_check_scope": "basic", "native_linker": "llvm" if sys.platform == "linux" else "system", "focused_regressions": ("core=" + self.core,)})


class NativeCliBatchTests(unittest.TestCase):
    stages = (("first group", ("first", "second")), ("last group", ("third",)))
    success = ("running 3 tests\ntest first ... ok\ntest second ... ok\ntest third ... ok\n\n"
               "test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s\n")

    @staticmethod
    def harness(root):
        harness = root / "harness"
        harness.write_text(f"#!{sys.executable}\n" +
            "import json,os,sys,time\nfrom pathlib import Path\n"
            "settings=json.loads(Path('settings.json').read_text())\n"
            "if settings.get('create_fixture'):\n"
            " phase=Path('listing' if '--list' in sys.argv else 'execution')\n"
            " phase.mkdir(mode=0o777)\n"
            " os.close(os.open(phase/'owned-lock',os.O_CREAT|os.O_WRONLY,0o666))\n"
            "row={'args':sys.argv[1:],'cwd':os.getcwd(),'marker':os.getenv('CLI_BATCH_TEST_MARKER')}\n"
            "if os.getenv('CLI_BATCH_TEST_FD'):\n row['fd_size']=os.fstat(int(os.environ['CLI_BATCH_TEST_FD'])).st_size\n"
            "with Path('calls.jsonl').open('a') as f:f.write(json.dumps(row)+'\\n')\n"
            "if '--list' in sys.argv:\n print(settings.get('listing','first: test\\nsecond: test\\nthird: test\\nunselected: test'));sys.exit(0)\n"
            "assert sys.stdin.read()==''\n"
            "time.sleep(settings.get('delay',0))\n"
            "sys.stdout.write(settings['stdout']);sys.stderr.write(settings.get('stderr',''))\n"
            "sys.exit(settings.get('code',0))\n")
        harness.chmod(0o700)
        return str(harness)

    def invoke(self, root, settings, *, env=None, locks=(), stages=None):
        (root / "settings.json").write_text(json.dumps(settings))
        output, errors = io.StringIO(), io.StringIO()
        failure = None
        with contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
            try:
                gate.run_stages(self.harness(root), root, dict(os.environ) if env is None else env,
                                self.stages if stages is None else stages, locks, batch=True)
            except gate.SelectedRegressionFailures as error:
                failure = error
        return failure, output.getvalue(), errors.getvalue()

    def test_exact_batch_preserves_cwd_environment_descriptors_and_closed_selection(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            held = root / "held"
            held.write_bytes(b"owned lock")
            with held.open("rb") as lock:
                env = dict(os.environ, CLI_BATCH_TEST_MARKER="present", CLI_BATCH_TEST_FD=str(lock.fileno()))
                failure, output, errors = self.invoke(root, {"stdout": self.success}, env=env, locks=(lock.fileno(),))
            self.assertIsNone(failure)
            self.assertEqual(errors, "")
            calls = [json.loads(line) for line in (root / "calls.jsonl").read_text().splitlines()]
            self.assertEqual(len(calls), 2)
            self.assertEqual(calls[0]["args"], ["--list", "--format", "terse"])
            self.assertEqual(calls[1]["args"], ["first", "second", "third", "--exact", "--test-threads=1", "--format", "pretty", "--color", "never"])
            self.assertTrue(all(row["cwd"] == str(root) and row["marker"] == "present" and row["fd_size"] == 10 for row in calls))
            self.assertIn("passed first group (2 tests in CLI batch)", output)
            self.assertIn("passed last group (1 tests in CLI batch)", output)

    def test_serial_and_batch_children_create_private_fixtures_with_permissive_parent_umask(self):
        for batch in (False, True):
            with self.subTest(batch=batch), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                (root / "settings.json").write_text(json.dumps({"create_fixture": True,
                    "stdout": "running 1 test\ntest first ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.01s\n"}))
                harness = self.harness(root)
                original_umask = os.umask(0o002)
                try:
                    with contextlib.redirect_stdout(io.StringIO()):
                        gate.run_stages(harness, root, dict(os.environ), (("fixture", ("first",)),), (), batch=batch)
                    self.assertEqual(os.umask(0o002), 0o002)
                finally:
                    os.umask(original_umask)
                for phase in ("listing", "execution"):
                    self.assertEqual(stat.S_IMODE((root / phase).stat().st_mode), 0o700)
                    self.assertEqual(stat.S_IMODE((root / phase / "owned-lock").stat().st_mode), 0o600)

    def test_empty_duplicate_or_missing_selection_cannot_execute_unselected_tests(self):
        for stages in ((), (("empty", ()),), (("duplicate", ("first", "first")),)):
            with self.subTest(stages=stages), patch.object(gate.subprocess, "run") as run:
                with self.assertRaisesRegex(gate.CheckError, "nonempty.*unique"):
                    gate.run_stages("/unused", Path("/unused"), {}, stages, (), batch=True)
                run.assert_not_called()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / "settings.json").write_text(json.dumps({"listing": "first: test", "stdout": self.success}))
            with self.assertRaisesRegex(gate.CheckError, "missing.*second.*third"):
                gate.run_stages(self.harness(root), root, dict(os.environ), self.stages, (), batch=True)
            self.assertEqual(len((root / "calls.jsonl").read_text().splitlines()), 1)

    def test_malformed_missing_ignored_measured_duplicate_unexpected_and_dishonest_results_fail(self):
        cases = {
            "missing footer": self.success.split("test result:")[0],
            "missing running count": self.success.replace("running 3 tests\n", ""),
            "wrong running count": self.success.replace("running 3", "running 2"),
            "missing named result": self.success.replace("test second ... ok\n", ""),
            "duplicate named result": self.success.replace("test second ... ok", "test first ... ok"),
            "unexpected named result": self.success.replace("test second ... ok", "test unselected ... ok"),
            "ignored": self.success.replace("test second ... ok", "test second ... ignored, not enabled").replace("3 passed; 0 failed; 0 ignored", "2 passed; 0 failed; 1 ignored"),
            "measured": self.success.replace("test second ... ok", "test second ... bench: 10 ns/iter").replace("3 passed; 0 failed; 0 ignored; 0 measured", "2 passed; 0 failed; 0 ignored; 1 measured"),
            "wrong passed count": self.success.replace("3 passed", "2 passed"),
            "wrong filtered count": self.success.replace("1 filtered out", "0 filtered out"),
            "contradictory status": self.success.replace("result: ok", "result: FAILED"),
            "partial result": self.success.replace("test second ... ok", "test second ... "),
            "unknown terminal": self.success.replace("test second ... ok", "test second ... SKIPPED"),
            "duplicate footer": self.success + self.success.split("\n\n")[1],
            "trailing output": self.success + "aborted after footer\n",
            "unexpected progress": self.success.replace("test second ... ok", "not a libtest result\ntest second ... ok"),
            "forged failure section": self.success.replace("test second ... ok", "failures:\ntest second ... ok"),
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            for label, stdout in cases.items():
                with self.subTest(label=label):
                    failure, _, errors = self.invoke(root, {"stdout": stdout, "stderr": "fixture diagnostic\n"})
                    self.assertIsNotNone(failure)
                    self.assertIn(stdout, errors)
                    self.assertIn("fixture diagnostic", errors)
            calls = [json.loads(line) for line in (root / "calls.jsonl").read_text().splitlines()]
            self.assertEqual(len(calls), 2 * len(cases), "each failed census executes once, never replays")

    def test_normal_failures_aggregate_without_admitting_captured_result_lookalikes(self):
        stdout = ("running 3 tests\ntest first ... FAILED\ntest second ... ok\ntest third ... FAILED\n"
                  "\nfailures:\n\n---- first stdout ----\n"
                  "test unselected ... ok\ntest first ... ok\n"
                  "test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s\n"
                  "\nfailures:\n    first\n    third\n\n"
                  "test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.02s\n")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            failure, output, errors = self.invoke(root, {"stdout": stdout, "code": 101})
            self.assertEqual(failure.failures, ("regression did not pass: first (FAILED)", "regression did not pass: third (FAILED)"))
            self.assertEqual(errors, stdout)
            self.assertNotIn("passed CLI batch", output)
            self.assertEqual(len((root / "calls.jsonl").read_text().splitlines()), 2)
            failure, _, _ = self.invoke(root, {"stdout": stdout, "code": 0})
            self.assertIn("success despite failed", str(failure))

    def test_abnormal_exit_reports_every_unexecuted_name_without_replaying_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            for code in (101, 70):
                with self.subTest(code=code):
                    failure, _, _ = self.invoke(root, {"stdout": self.success, "code": code})
                    self.assertIn(f"exit {code}", str(failure))
            failure, _, errors = self.invoke(root, {"stdout": "running 3 tests\ntest first ... ok\ntest second ... ", "code": 70})
            self.assertIn("terminal result: second", str(failure))
            self.assertIn("terminal result: third", str(failure))
            self.assertNotIn("terminal result: first", str(failure))
            self.assertIn("test first ... ok", errors)
            self.assertEqual(len((root / "calls.jsonl").read_text().splitlines()), 6)

    def test_quiet_batch_has_bounded_progress_without_cargo_labels_or_census_changes(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.object(gate, "NATIVE_TEST_PROGRESS_INTERVAL_SECONDS", 0.01):
            failure, output, errors = self.invoke(Path(directory), {"stdout": self.success, "delay": 0.06})
            self.assertIsNone(failure)
            self.assertIn("CLI batch running (3 tests;", output)
            self.assertNotIn("taira-cargo", output)
            self.assertEqual(errors, "")


class EarlyReleaseCheckTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)
        metadata = patch.object(gate, "check_test_harnesses")
        metadata.start()
        self.addCleanup(metadata.stop)
        mock = patch.object(gate, "run_pure_fsm_checks")
        self.pure_fsm = mock.start()
        self.addCleanup(mock.stop)
        source = patch.object(gate, "run_lifecycle_source_checks")
        self.lifecycle_source = source.start()
        self.addCleanup(source.stop)
        config = patch.object(gate, "run_config_checks")
        self.config = config.start()
        self.addCleanup(config.stop)
        capacity = patch.object(gate, "require_network_fixture_capacity")
        self.capacity = capacity.start()
        self.addCleanup(capacity.stop)

    def test_failed_regression_does_not_hide_later_independent_failures(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            harness = root / "harness"
            harness.write_text(f"#!{sys.executable}\n" + "import sys\nfrom pathlib import Path\n"
                "if '--list' in sys.argv:\n print('first: test\\nsecond: test\\nthird: test');sys.exit(0)\n"
                "name=sys.argv[1]\n"
                "with Path('executed').open('a') as f:f.write(name+'\\n')\n"
                "print('test '+name+(' ... ok' if name=='second' else ' ... FAILED'))\n"
                "print('test result: ok. 1 passed; 0 failed; 0 ignored;' if name=='second' else 'fixture failure')\n"
                "sys.exit(0 if name=='second' else 101)\n")
            harness.chmod(0o700)
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, '2 selected regressions failed:.*first.*third'):
                    gate.run_stages(str(harness), root, dict(os.environ), (("independent fixtures", ("first", "second", "third")),), ())
            self.assertEqual((root / "executed").read_text().splitlines(), ['first', 'second', 'third'])

    def test_command_reuses_native_cargo_lane_and_does_not_run_full_suite(self):
        self.assertEqual(gate.compile_command(Path("/repo"), {"CARGO": "/fixed/cargo"}), [
            "/fixed/cargo", "--config", "/repo/.cargo/config.toml", "test",
            "--manifest-path", "/repo/Cargo.toml", "--locked", "--offline", "-p", "iroha_cli",
            "--bin", "iroha", "--no-run", "--message-format=json-render-diagnostics",
        ])

    def test_artifact_accepts_only_executable_iroha_binary_test_harness(self):
        event = {"reason": "compiler-artifact", "target": {"name": "iroha", "kind": ["bin"]},
                 "profile": {"test": True}, "executable": "/warm/debug/deps/iroha-test"}
        self.assertEqual(gate.test_artifact(json.dumps(event)), event["executable"])
        for changes in ({"profile": {"test": False}}, {"executable": None},
                        {"target": {"name": "iroha", "kind": ["lib"]}},
                        {"target": {"name": "kagami", "kind": ["bin"]}}):
            self.assertIsNone(gate.test_artifact(json.dumps(event | changes)))
        self.assertIsNone(gate.test_artifact("[cargo-fast] normal progress"))

    def test_torii_selection_requires_shipping_external_contract_harness(self):
        command = gate.compile_command(Path("/repo"), {"CARGO": "/fixed/cargo"}, harness="torii")
        self.assertIn("iroha_torii", command)
        self.assertIn("taira_app_contracts", command)
        self.assertNotIn("--lib", command)
        self.assertNotIn("--no-default-features", command)
        event = {"reason": "compiler-artifact", "target": {"name": "taira_app_contracts", "kind": ["test"]},
                 "profile": {"test": True}, "executable": "/warm/taira-contracts"}
        self.assertEqual(gate.test_artifact(json.dumps(event), harness="torii"), event["executable"])
        self.assertIsNone(gate.test_artifact(json.dumps(event)))
        for changes in ({"profile": {"test": False}}, {"executable": None},
                        {"target": {"name": "torii_core_routes", "kind": ["test"]}},
                        {"target": {"name": "taira_app_contracts", "kind": ["lib"]}}):
            self.assertIsNone(gate.test_artifact(json.dumps(event | changes), harness="torii"))

    def test_core_selection_requires_the_actual_library_test_artifact(self):
        command = gate.compile_command(Path("/repo"), {"CARGO": "/fixed/cargo"}, harness="core")
        self.assertEqual(command[8:11], ["-p", "iroha_core", "--lib"])
        event = {"reason": "compiler-artifact", "target": {"name": "iroha_core", "kind": ["lib"]},
                 "profile": {"test": True}, "executable": "/warm/core-contracts"}
        self.assertEqual(gate.test_artifact(json.dumps(event), harness="core"), event["executable"])
        self.assertIsNone(gate.test_artifact(json.dumps(event)))
        self.assertIsNone(gate.test_artifact(json.dumps(event | {"profile": {"test": False}}), harness="core"))
        for selection in ({"harness": "unreviewed"}, {"harness": ""}):
            with self.assertRaises(gate.CheckError):
                gate.compile_command(Path("/repo"), {"CARGO": "/fixed/cargo"}, **selection)

    def test_transport_selections_require_their_actual_library_test_artifacts(self):
        for harness, package in (("torii-shared", "iroha_torii_shared"), ("crypto", "iroha_crypto"), ("p2p", "iroha_p2p"),
                                 ("test-network", "iroha_test_network")):
            with self.subTest(harness=harness):
                command = gate.compile_command(Path("/repo"), {"CARGO": "/fixed/cargo"}, harness=harness)
                self.assertEqual(command[8:11], ["-p", package, "--lib"])
                self.assertNotIn("--no-default-features", command)
                event = {"reason": "compiler-artifact", "target": {"name": package, "kind": ["lib"]},
                         "profile": {"test": True}, "executable": "/warm/" + package}
                self.assertEqual(gate.test_artifact(json.dumps(event), harness=harness), event["executable"])
                for changes in ({"profile": {"test": False}}, {"executable": None},
                                {"target": {"name": package, "kind": ["bin"]}},
                                {"target": {"name": "unrelated", "kind": ["lib"]}}):
                    self.assertIsNone(gate.test_artifact(json.dumps(event | changes), harness=harness))

    def test_failed_build_preserves_rendered_compiler_error(self):
        diagnostic = "error[E0308]: synthetic fixture type mismatch\n"
        event = {"reason": "compiler-message", "message": {"rendered": diagnostic}}
        child = MagicMock()
        child.stdout = io.StringIO("[cargo-fast] warm lane\n" + json.dumps(event) + "\n")
        child.wait.return_value = 101
        process = MagicMock()
        process.__enter__.return_value = child
        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.object(gate.subprocess, "Popen", return_value=process), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            with self.assertRaisesRegex(gate.CheckError, "native CLI build failed"):
                gate.compile_harness(Path("/fixture-only"), {"CARGO": "/fixed/cargo"})
        self.assertEqual(stderr.getvalue(), diagnostic)
        self.assertIn("[cargo-fast] warm lane", stdout.getvalue())
        self.assertNotIn("compiler-message", stdout.getvalue())

    def test_missing_or_renamed_regression_is_fatal(self):
        names = [name for _, tests in gate.STAGES for name in tests]
        gate.require_tests("\n".join(f"{name}: test" for name in names))
        for listing in ("", "\n".join(f"{name}: test" for name in names[:-1])):
            with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                gate.require_tests(listing)

    def test_selection_has_no_duplicate_test_names(self):
        names = [name for _, tests in gate.STAGES for name in tests]
        self.assertEqual(len(names), len(set(names)))
        self.assertEqual(gate.STAGES[0], ("core canary command composition", (
            "taira_public_reset::host::tests::coordinator_write_canary_argv_passes_child_validation_for_all_core_actions",
            "taira::tests::final_canary_predecessor_requires_its_independent_faucet_policy",
            "taira::tests::write_canary_policy_inputs_are_operation_and_action_scoped",
        )))
        self.assertIn(
            "taira_public_reset::host::tests::candidate_operator_status_child_binds_both_inherited_signers",
            names,
        )
        self.assertIn(
            "taira_public_reset::config::tests::operator_keygen_publishes_canonical_private_key_and_only_public_report",
            names,
        )

    def test_transport_selectors_are_mandatory_unique_and_fail_when_missing(self):
        for stages in (gate.CONFIG_STAGES, gate.CRYPTO_STAGES, gate.P2P_STAGES, gate.TEST_NETWORK_STAGES):
            names = [name for _, tests in stages for name in tests]
            self.assertTrue(names)
            self.assertEqual(len(names), len(set(names)))
            gate.require_tests("\n".join(f"{name}: test" for name in names), stages)
            with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                gate.require_tests("\n".join(f"{name}: test" for name in names[1:]), stages)

    def test_native_devex_scope_controller_and_absence_contracts_are_mandatory(self):
        required = {'cli': ['taira_public_reset::executor_model::tests::occupied_service_state_is_explicit_strict_and_signed',
                 'taira_public_reset::executor_model::tests::stopped_state_identity_survives_archive_restore_and_rejects_substitution',
                 'taira_public_reset::host::tests::stopped_predecessor_absence_checks_cgroup_and_escaped_references',
                 'taira_public_reset::host::tests::prior_service_state_never_restarts_stopped_or_falls_back_from_running',
                 'taira_public_reset::host::occupied::tests::stopped_unit_admission_requires_the_exact_prior_or_durable_successor',
                 'taira_public_reset::executor_model::tests::qualification_scope_requires_exact_nullable_inrou_closure',
                 'taira_public_reset::executor_model::tests::qualification_scope_reopens_exact_durable_execution_boundaries',
                 'taira_public_reset::inputs::tests::candidate_runtime_scope_requires_disabled_core_and_exact_full_owner',
                 'taira_dataspace_deploy::tests::manifest_binds_native_identity_and_spending_limits',
                 'taira_dataspace_deploy::tests::operation_id_is_stable_for_equivalent_intent',
                 'taira_dataspace_deploy::tests::catalog_transition_preserves_sparse_baseline_and_cas',
                 'taira_dataspace_deploy::tests::retained_phase_rejects_changed_wire_owner_fee_or_instructions',
                 'taira_dataspace_deploy::tests::namespace_plan_requires_two_bounded_paid_creates',
                 'taira_dataspace_deploy::tests::journal_dispatch_claim_is_durable_and_exclusive',
                 'taira_dataspace_deploy::tests::journal_rejects_links_replacement_and_incomplete_records',
                 'taira_dataspace_deploy::tests::status_requires_exact_global_and_peer_state_applied',
                 'taira_dataspace_deploy::tests::init_builds_native_restricted_intent_from_policy_and_profile',
                 'taira_dataspace_deploy::tests::init_rejects_policy_drift_and_parses_explicit_caps',
                 'taira_dataspace_deploy::lane_manifest::tests::generates_typed_manifest_from_executed_signed_genesis',
                 'taira_dataspace_deploy::lane_manifest::tests::rejects_genesis_without_explicit_bindings_or_with_malformed_wire',
                 'taira_dataspace_deploy::lane_manifest::tests::requires_unique_complete_activated_genesis_bindings',
                 'taira_dataspace_deploy::lane_manifest::tests::rejects_changed_binding_and_non_four_trusted_roster',
                 'taira_dataspace_deploy::finality::tests::deployment_trust_derives_exact_genesis_roster_and_network',
                 'taira_dataspace_deploy::finality::tests::deployment_trust_rejects_wrong_network_key_and_changed_genesis_wire',
                 'taira_dataspace_deploy::finality::tests::deployment_trust_requires_four_distinct_genesis_peers_and_public_endpoints',
                 'taira_public_reset::host::stopped_runtime::tests::stopped_owner_projection_ignores_unrelated_credentials_and_keeps_typed_defaults',
                 'taira_public_reset::deployment_profile::tests::deployment_profile_binds_native_genesis_and_ordered_inventory_peers',
                 'taira_public_reset::deployment_profile::tests::deployment_profile_rejects_genesis_artifact_peer_and_slot_substitution',
                 'taira_public_reset::deployment_profile::tests::deployment_profile_command_parses_without_private_or_runtime_arguments'],
         'core': ['zk::zkparse::production_parameter_cache_tests::finite_production_cache_initializes_once_across_threads',
                  'zk::zkparse::production_parameter_cache_tests::finite_production_cache_matches_native_parameter_bytes_and_fingerprint',
                  'zk::zkparse::production_parameter_cache_tests::finite_production_cache_rejects_unadmitted_domains_without_construction',
                  'zk::halo2_ipa_parameter_source_tests::production_parameter_source_rejects_duplicate_and_mismatched_metadata',
                  'zk::halo2_ipa_parameter_source_tests::production_parameter_source_rejects_unbounded_k_before_construction',
                  'zk::debug_backend_tests::halo2_ivm_execution_rejects_relabelled_demo_verifying_key',
                  'sns::tests::registration_absence_is_distinct_from_policy_and_malformed_state'],
         'torii-unit': ['sns::tests::registration_absence_http_response_is_typed_and_other_not_found_is_not',
                        'openapi::tests::sns_name_absence_openapi_is_typed_and_selector_bound'],
         'torii-shared': ['sns::tests::missing_registration_response_requires_exact_fields_and_selector'],
         'client': ['sns::tests::optional_name_http_absence_requires_exact_typed_json',
                    'sns::tests::optional_name_http_success_binds_canonical_namespace_and_record',
                    'client::evidence_http_tests::bridge_finality_attestation_reader_binds_exact_request_headers_and_signed_body',
                    'client::evidence_http_tests::bridge_finality_attestation_reader_rejects_wrong_bindings_and_invalid_http_body']}
        for scope in ("basic", "full"):
            selected = gate.qualification_stages(scope)
            for harness, tests in required.items():
                actual = [test for _, tests in selected[harness] for test in tests]
                for test in tests:
                    with self.subTest(scope=scope, harness=harness, test=test):
                        self.assertEqual(actual.count(test), 1)
            _, graph, _ = gate.native_harness_plan(selected, ())
            self.assertIn("torii-shared", graph)
        self.assertEqual(gate.HARNESS_TARGETS["torii-shared"][3], ["-p", "iroha_torii_shared", "--lib"])
        focused = gate.focused_regression_stages("basic", ["torii-shared=" + required["torii-shared"][0]])
        self.assertEqual(tuple(focused), ("torii-shared",))

    def test_complete_regression_census_tracks_every_native_stage_group(self):
        self.assertEqual(gate.selected_regression_count("full"), EXPECTED_REGRESSION_COUNT)
        for group in ("MV_OWNERSHIP_STAGES", "MV_EBR_STAGES", "MV_MAP_STAGES", "MV_ADMITTED_MAP_STAGES", "CONCREAD_STAGES", "STAGES", "CONFIG_STAGES", "CONFIG_UNIT_STAGES", "DATA_MODEL_STAGES", "CRYPTO_STAGES", "P2P_STAGES", "CORE_STAGES",
                      "TEST_NETWORK_STAGES", "NETWORK_STAGES", "PROOF_STAGES",
                      "PROOF_FLOW_STAGES", "TORII_STAGES", "TORII_SHARED_STAGES", "CLIENT_STAGES", "WALLET_STAGES", "TORII_UNIT_STAGES", "DAEMON_STAGES", "KAGAMI_STAGES"):
            original_count = sum(len(names) for _, names in getattr(gate, group))
            with self.subTest(group=group), patch.object(gate, group, (("fixture", ("one", "two")),)):
                self.assertEqual(gate.selected_regression_count("full"), EXPECTED_REGRESSION_COUNT - original_count + 2)

    def test_exact_one_test_passes(self):
        result = subprocess.CompletedProcess([], 0,
            "test example ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 99 filtered out\n", "")
        gate.require_one_pass("example", result)

    def test_zero_ignored_wrong_or_failed_test_is_fatal(self):
        outputs = (
            (0, "test result: ok. 0 passed; 0 failed; 0 ignored; 100 filtered out\n"),
            (0, "test example ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored;\n"),
            (0, "test another ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n"),
            (101, "test example ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored;\n"),
        )
        for code, output in outputs:
            with self.subTest(code=code, output=output), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "did not execute and pass"):
                    gate.require_one_pass("example", subprocess.CompletedProcess([], code, output, ""))


    def test_frozen_harness_uses_captured_manifest_config_and_no_git_lookup(self):
        child = MagicMock()
        child.stdout = io.StringIO(json.dumps({"reason": "compiler-artifact", "target": {"name": "iroha", "kind": ["bin"]}, "profile": {"test": True}, "executable": "/warm/iroha-test"}) + "\n")
        child.wait.return_value = 0
        process = MagicMock()
        process.__enter__.return_value = child
        with patch.object(gate.subprocess, "Popen", return_value=process) as spawn, \
             patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: {name: row["executable"] for name, row in rows.items()}), contextlib.redirect_stdout(io.StringIO()):
            gate.compile_harness(Path("/frozen"), {"CARGO": "/fixed/cargo"}, lock_fds=(77, 88))
        self.assertEqual(spawn.call_args.args[0][:4], ["/fixed/cargo", "--config", "/frozen/.cargo/config.toml", "test"])
        self.assertEqual(spawn.call_args.kwargs["cwd"], "/")
        self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77, 88))
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch.object(gate.subprocess, "check_output", side_effect=AssertionError("must not inspect mutable Git")))
            compile = stack.enter_context(patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies("/fixture/harness")))
            run = stack.enter_context(patch.object(gate.subprocess, "run", side_effect=[subprocess.CompletedProcess([], 0, "fixture: test\n", ""), subprocess.CompletedProcess([], 0, "running 1 test\ntest fixture ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n", "")]))
            stack.enter_context(patch.object(gate, "STAGES", (("fixtures", ("fixture",)),)))
            stack.enter_context(patch.multiple(gate, MV_OWNERSHIP_STAGES=(),
                                               MV_EBR_STAGES=(), MV_MAP_STAGES=(), MV_ADMITTED_MAP_STAGES=(), CONCREAD_STAGES=(),
                                               CONFIG_UNIT_STAGES=()))
            stack.enter_context(patch.object(gate, "DATA_MODEL_STAGES", ()))
            stack.enter_context(patch.object(gate, "CRYPTO_STAGES", ()))
            stack.enter_context(patch.object(gate, "P2P_STAGES", ()))
            stack.enter_context(patch.object(gate, "CORE_STAGES", ()))
            stack.enter_context(patch.object(gate, "DAEMON_STAGES", ()))
            stack.enter_context(patch.object(gate, "CLIENT_STAGES", ()))
            stack.enter_context(patch.object(gate, "WALLET_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_UNIT_STAGES", ()))
            stack.enter_context(patch.object(gate, "TEST_NETWORK_STAGES", ()))
            stack.enter_context(patch.object(gate, "NETWORK_STAGES", ()))
            stack.enter_context(patch.object(gate, "PROOF_STAGES", ()))
            stack.enter_context(patch.object(gate, "PROOF_FLOW_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_SHARED_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_LIFECYCLE_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_STAGES", ()))
            stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
            gate.run_checks(Path("/frozen"), qualification_scope="full", environment={"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}, source_commit="a" * 40, lock_fds=(77, 88))
        self.assertEqual([call.kwargs["cwd"] for call in run.call_args_list], [Path("/warm"), Path("/warm")])
        self.assertNotIn("frozen", compile.call_args.kwargs)
        self.assertEqual(compile.call_args.args[1]["VERGEN_GIT_SHA"], "a" * 40)


    def test_mutable_check_keeps_git_checks_and_inherits_lane_lock_in_every_child(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/routine", "CARGO_INCREMENTAL": "0"}
        results = [subprocess.CompletedProcess([], 0, "fixture: test\n", ""),
                   subprocess.CompletedProcess([], 0, "running 1 test\ntest fixture ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n", "")]
        with contextlib.ExitStack() as stack:
            git = stack.enter_context(patch.object(gate.subprocess, "check_output", return_value="a" * 40))
            compile = stack.enter_context(patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies("/fixture/harness")))
            run = stack.enter_context(patch.object(gate.subprocess, "run", side_effect=results))
            stack.enter_context(patch.object(gate, "STAGES", (("fixtures", ("fixture",)),)))
            stack.enter_context(patch.multiple(gate, MV_OWNERSHIP_STAGES=(),
                                               MV_EBR_STAGES=(), MV_MAP_STAGES=(), MV_ADMITTED_MAP_STAGES=(), CONCREAD_STAGES=(),
                                               CONFIG_UNIT_STAGES=()))
            stack.enter_context(patch.object(gate, "DATA_MODEL_STAGES", ()))
            stack.enter_context(patch.object(gate, "CRYPTO_STAGES", ()))
            stack.enter_context(patch.object(gate, "P2P_STAGES", ()))
            stack.enter_context(patch.object(gate, "CORE_STAGES", ()))
            stack.enter_context(patch.object(gate, "DAEMON_STAGES", ()))
            stack.enter_context(patch.object(gate, "CLIENT_STAGES", ()))
            stack.enter_context(patch.object(gate, "WALLET_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_UNIT_STAGES", ()))
            stack.enter_context(patch.object(gate, "TEST_NETWORK_STAGES", ()))
            stack.enter_context(patch.object(gate, "NETWORK_STAGES", ()))
            stack.enter_context(patch.object(gate, "PROOF_STAGES", ()))
            stack.enter_context(patch.object(gate, "PROOF_FLOW_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_SHARED_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_LIFECYCLE_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_STAGES", ()))
            stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
            gate.run_checks(Path("/mutable"), qualification_scope="full", environment=env, lock_fds=(77,))
        self.assertEqual(git.call_count, 2)
        self.assertTrue(all(call.kwargs["env"]["CARGO_HOME"] == "/isolated" for call in git.call_args_list))
        self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,), "harnesses": ("config", "cli")})
        self.assertTrue(all(call.kwargs["pass_fds"] == (77,) for call in run.call_args_list))
        self.assertTrue(all(call.kwargs["cwd"] == Path("/mutable") for call in run.call_args_list))
        self.assertEqual(env, {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/routine", "CARGO_INCREMENTAL": "0"})
        self.assertEqual(compile.call_args.args[1]["CARGO_INCREMENTAL"], "0")
        self.assertTrue(all(call.kwargs["env"]["CARGO_INCREMENTAL"] == "0" for call in run.call_args_list))

    def test_torii_contract_failure_prevents_overall_pass_and_keeps_same_custody(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        results = [subprocess.CompletedProcess([], 0, "cli: test\n", ""),
                   subprocess.CompletedProcess([], 0, "running 1 test\ntest cli ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n", ""),
                   subprocess.CompletedProcess([], 0, "route: test\n", ""),
                   subprocess.CompletedProcess([], 101, "test route ... FAILED\n", "")]
        output = io.StringIO()
        with contextlib.ExitStack() as stack:
            compile = stack.enter_context(patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies({"torii": "/warm/routes", "cli": "/warm/cli"})))
            run = stack.enter_context(patch.object(gate.subprocess, "run", side_effect=results))
            stack.enter_context(patch.object(gate, "STAGES", (("CLI", ("cli",)),)))
            stack.enter_context(patch.multiple(gate, MV_OWNERSHIP_STAGES=(),
                                               MV_EBR_STAGES=(), MV_MAP_STAGES=(), MV_ADMITTED_MAP_STAGES=(), CONCREAD_STAGES=(),
                                               CONFIG_UNIT_STAGES=()))
            stack.enter_context(patch.object(gate, "DATA_MODEL_STAGES", ()))
            stack.enter_context(patch.object(gate, "CRYPTO_STAGES", ()))
            stack.enter_context(patch.object(gate, "P2P_STAGES", ()))
            stack.enter_context(patch.object(gate, "CORE_STAGES", ()))
            stack.enter_context(patch.object(gate, "DAEMON_STAGES", ()))
            stack.enter_context(patch.object(gate, "CLIENT_STAGES", ()))
            stack.enter_context(patch.object(gate, "WALLET_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_UNIT_STAGES", ()))
            stack.enter_context(patch.object(gate, "TEST_NETWORK_STAGES", ()))
            stack.enter_context(patch.object(gate, "NETWORK_STAGES", ()))
            stack.enter_context(patch.object(gate, "PROOF_STAGES", ()))
            stack.enter_context(patch.object(gate, "PROOF_FLOW_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_SHARED_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_LIFECYCLE_STAGES", ()))
            stack.enter_context(patch.object(gate, "TORII_STAGES", (("Torii", ("route",)),)))
            stack.enter_context(contextlib.redirect_stdout(output))
            stack.enter_context(contextlib.redirect_stderr(io.StringIO()))
            with self.assertRaisesRegex(gate.CheckError, "route.*exit 101"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        self.assertEqual(compile.call_count, 1)
        self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,), "harnesses": ("config", "torii", "cli")})
        self.assertTrue(all(call.kwargs["cwd"] == Path("/warm") and call.kwargs["pass_fds"] == (77,)
                            for call in run.call_args_list))
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_cli_contract_infrastructure_failure_stops_after_startup_before_other_groups(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        output = io.StringIO()
        def fail_cli(harness, root, env, stages, locks, **_kwargs):
            if stages == gate.STAGES:
                raise gate.CheckError("public transaction stalled")
        with patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies({
                name: "/warm/" + name for name in gate.HARNESS_TARGETS})) as compile, \
             patch.object(gate, "run_network_checks") as network, \
             patch.multiple(gate, MV_OWNERSHIP_STAGES=(), MV_EBR_STAGES=(),
                            MV_MAP_STAGES=(), MV_ADMITTED_MAP_STAGES=(), CONCREAD_STAGES=(), CONFIG_UNIT_STAGES=()), \
             patch.object(gate, "DATA_MODEL_STAGES", ()), \
             patch.object(gate, "CRYPTO_STAGES", ()), \
             patch.object(gate, "P2P_STAGES", ()), \
             patch.object(gate, "run_stages", side_effect=fail_cli) as run, \
             contextlib.redirect_stdout(output):
            with self.assertRaisesRegex(gate.CheckError, "public transaction stalled"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        self.assertEqual(compile.call_count, 1)
        network.assert_not_called()
        self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,), "harnesses": ("config", "proof", "proof-flows", "core", "test-network", "client", "wallet", "torii-unit", "torii", "torii-shared", "torii-lifecycle", "daemon", "network", "cli")})
        self.assertEqual([call.args[0] for call in run.call_args_list],
                         ["/warm/core", "/warm/core", "/warm/torii-unit", "/warm/daemon", "/warm/cli"])
        self.assertEqual(run.call_args.args[3], gate.STAGES)
        self.assertEqual(run.call_args.args[4], (77,))
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_network_failure_does_not_trigger_separate_harness_builds(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        names = ("config", "mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread", "config-unit", "data-model", "proof", "proof-flows", "crypto", "p2p", "core", "test-network", "client", "wallet", "torii-unit", "torii", "torii-shared", "torii-lifecycle", "daemon", "network", "cli")
        with patch.object(gate, "run_network_checks", side_effect=gate.CheckError("consensus stalled")) as network, \
             patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies({
                 name: "/warm/" + name for name in names})) as batch, \
             patch.object(gate, "compile_harness", return_value=FixtureCopies("/warm/torii")) as compile, \
             patch.object(gate, "run_stages") as stages, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "consensus stalled"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        network.assert_called_once_with(Path("/frozen"), Path("/warm"),
            env | {"VERGEN_GIT_SHA": "a" * 40, "IROHA_GIT_COMMIT_HASH": "a" * 40}, (77,),
            harness="/warm/network", stages=gate.NETWORK_STAGES)
        self.assertEqual(batch.call_count, 1)
        self.assertEqual(batch.call_args.kwargs, {"lock_fds": (77,), "harnesses": names})
        compile.assert_not_called()
        self.assertEqual([call.args[0] for call in stages.call_args_list], ["/warm/" + name for name in ("mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread", "core", "core", "torii-unit", "daemon", "cli") + names[6:-2]])
        self.assertEqual([call.kwargs for call in stages.call_args_list],
                         [{"batch": True} if call.args[0] == "/warm/cli" else {} for call in stages.call_args_list])
        self.assertEqual([call.args[3] for call in stages.call_args_list],
                         [gate.MV_OWNERSHIP_STAGES, gate.MV_EBR_STAGES, gate.MV_MAP_STAGES, gate.MV_ADMITTED_MAP_STAGES, gate.CONCREAD_STAGES, gate.CORE_PENDING_KURA_RECOVERY_STAGES, tuple(stage for stage in gate.CORE_STARTUP_STAGES if stage not in gate.CORE_PENDING_KURA_RECOVERY_STAGES), gate.TORII_STARTUP_STAGES, gate.DAEMON_STARTUP_STAGES, gate.STAGES, gate.CONFIG_UNIT_STAGES, gate.DATA_MODEL_STAGES, gate.PROOF_STAGES, gate.PROOF_FLOW_STAGES, gate.CRYPTO_STAGES, gate.P2P_STAGES, tuple(stage for stage in gate.CORE_STAGES if stage not in gate.CORE_STARTUP_STAGES), gate.TEST_NETWORK_STAGES, gate.CLIENT_STAGES, gate.WALLET_STAGES, tuple(stage for stage in gate.TORII_UNIT_STAGES if stage not in gate.TORII_STARTUP_STAGES), gate.TORII_STAGES, gate.TORII_SHARED_STAGES, gate.TORII_LIFECYCLE_STAGES, tuple(stage for stage in gate.DAEMON_STAGES if stage not in gate.DAEMON_STARTUP_STAGES)])

    def test_transport_or_fixture_failure_stops_before_network_and_release_success(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        names = ("config", "mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread", "config-unit", "data-model", "proof", "proof-flows", "crypto", "p2p", "core", "test-network", "client", "wallet", "torii-unit", "torii", "torii-shared", "torii-lifecycle", "daemon", "network", "cli")
        for failed, expected in (("crypto", ["core", "core", "torii-unit", "daemon", "cli", "config-unit", "data-model", "proof", "proof-flows", "crypto"]),
                                 ("p2p", ["core", "core", "torii-unit", "daemon", "cli", "config-unit", "data-model", "proof", "proof-flows", "crypto", "p2p"]),
                                 ("fixture", ["core", "core", "torii-unit", "daemon", "cli", "config-unit", "data-model", "proof", "proof-flows", "crypto", "p2p", "core", "test-network"])):
            expected = ["mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread", *expected]
            outcomes = [None] * (len(expected) - 1) + [gate.CheckError(failed + " failed")]
            output = io.StringIO()
            with self.subTest(failed=failed), \
                 patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies({
                     name: "/warm/" + name for name in names})) as compile, \
                 patch.object(gate, "run_stages", side_effect=outcomes) as run, \
                 patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(output):
                with self.assertRaisesRegex(gate.CheckError, failed + " failed"):
                    gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
            self.assertEqual(compile.call_count, 1)
            self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,), "harnesses": names})
            self.assertEqual(compile.call_args.args[0], Path("/frozen"))
            self.assertEqual(compile.call_args.args[1]["CARGO_TARGET_DIR"], "/warm")
            self.assertEqual([call.args[0] for call in run.call_args_list], ["/warm/" + name for name in expected])
            network.assert_not_called()
            for call in run.call_args_list:
                self.assertEqual(call.args[1], Path("/warm"))
                self.assertEqual(call.args[4], (77,))
            self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_public_contract_library_failures_stop_before_http_and_node_builds(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        names = ("config", "mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread", "config-unit", "data-model", "proof", "proof-flows", "crypto", "p2p", "core", "test-network", "client", "wallet", "torii-unit", "torii", "torii-shared", "torii-lifecycle", "daemon", "network", "cli")
        for failed in ("client", "wallet", "torii-unit", "torii", "torii-shared", "torii-lifecycle"):
            def run(harness, *args, **_kwargs):
                if harness == "/warm/" + failed:
                    raise gate.CheckError(failed + " failed")
            with self.subTest(failed=failed), \
                 patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies({name: "/warm/" + name for name in names})), \
                 patch.object(gate, "run_stages", side_effect=run), \
                 patch.object(gate, "compile_harness") as other, \
                 patch.object(gate, "run_network_checks") as network, \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, failed + " failed"):
                    gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40)
            other.assert_not_called()
            network.assert_not_called()

    def test_unisolated_low_level_check_is_rejected_before_git_or_cargo(self):
        with patch.object(gate.subprocess, "check_output") as git, patch.object(gate, "compile_harness") as compile:
            with self.assertRaisesRegex(gate.CheckError, "isolated Cargo environment"):
                gate.run_checks(Path("/mutable"), qualification_scope="full", environment={})
        git.assert_not_called()
        compile.assert_not_called()

    def test_network_build_requires_distinct_standard_and_taira_shipping_launchers(self):
        events = [{"reason": "compiler-artifact", "target": {"name": name, "kind": ["bin"]},
                   "profile": {"test": False}, "executable": "/warm/" + name}
                  for name in ("iroha3d", "iroha", "iroha3d_taira")]
        for selected, accepted in ((events, True), (events[:1], False),
                                   ([event | {"profile": {"test": True}} for event in events], False)):
            child = MagicMock()
            child.stdout = io.StringIO("\n".join(json.dumps(event) for event in selected))
            child.wait.return_value = 0
            process = MagicMock()
            process.__enter__.return_value = child
            with patch.object(gate.subprocess, "Popen", return_value=process) as spawn, \
             patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: {name: row["executable"] for name, row in rows.items()}), contextlib.redirect_stdout(io.StringIO()):
                if accepted:
                    result = gate.compile_network_binaries(Path("/frozen"), {"CARGO": "/fixed/cargo"}, (77,))
                    self.assertEqual(result, {"iroha3d": "/warm/iroha3d", "iroha": "/warm/iroha", "taira-launcher": "/warm/iroha3d_taira"})
                else:
                    with self.assertRaisesRegex(gate.CheckError, "every required executable artifact"):
                        gate.compile_network_binaries(Path("/frozen"), {"CARGO": "/fixed/cargo"}, (77,))
            self.assertEqual(spawn.call_args.kwargs["cwd"], "/")
            self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77,))
            self.assertIn("iroha3d_taira", spawn.call_args.args[0])

    def test_beacon_fixture_daemon_has_a_separate_explicit_feature_build(self):
        event = {"reason": "compiler-artifact", "target": {"name": "iroha3d", "kind": ["bin"]},
                 "profile": {"test": False}, "executable": "/warm/iroha3d"}
        child = MagicMock()
        child.stdout = io.StringIO(json.dumps(event))
        child.wait.return_value = 0
        process = MagicMock()
        process.__enter__.return_value = child
        with patch.object(gate.subprocess, "Popen", return_value=process) as spawn, \
             patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: rows), \
             contextlib.redirect_stdout(io.StringIO()):
            records = gate.compile_network_binaries(Path("/frozen"), {"CARGO": "/fixed/cargo"},
                                                     (77,), message_control=True)
        self.assertEqual(set(records), {"iroha3d-message-control"})
        command = spawn.call_args.args[0]
        self.assertEqual(command[command.index("--features") + 1], "irohad/test-network-message-control")
        self.assertEqual(command.count("--bin"), 1)
        self.assertNotIn("iroha3d_taira", command)
        self.assertNotIn("--target-dir", command)
        self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77,))

    def test_beacon_fixture_output_requires_private_direct_non_git_root(self):
        with tempfile.TemporaryDirectory(dir=Path.home().resolve()) as temporary:
            parent = Path(temporary).resolve()
            private = parent / "private"
            with patch.dict(os.environ, {"TAIRA_TESTNET_BEACON_FIXTURE_DIR": str(private)}):
                self.assertEqual(gate.beacon_fixture_root(), private)
                self.assertEqual(stat.S_IMODE(private.stat().st_mode), 0o700)
                private.chmod(0o755)
                with self.assertRaisesRegex(gate.CheckError, "owner-only"):
                    gate.beacon_fixture_root()
                private.chmod(0o700)
                subprocess.run(["git", "init", "--quiet", str(private)], check=True)
                with self.assertRaisesRegex(gate.CheckError, "outside a Git"):
                    gate.beacon_fixture_root()
            alias = parent / "alias"
            alias.symlink_to(private, target_is_directory=True)
            with patch.dict(os.environ, {"TAIRA_TESTNET_BEACON_FIXTURE_DIR": str(alias)}):
                with self.assertRaisesRegex(gate.CheckError, "absolute direct path"):
                    gate.beacon_fixture_root()
            with patch.dict(os.environ, {"TAIRA_TESTNET_BEACON_FIXTURE_DIR": "relative"}):
                with self.assertRaisesRegex(gate.CheckError, "absolute direct path"):
                    gate.beacon_fixture_root()

    def test_network_gate_forbids_fallback_builds_and_sandbox_skips(self):
        env = {"CARGO_TARGET_DIR": "/warm"}
        with patch.object(gate, "compile_network_binaries", return_value=FixtureCopies({"iroha3d": "/warm/node", "iroha": "/warm/client", "taira-launcher": "/warm/taira", "kagami": "/warm/kagami", "iroha3d-message-control": "/warm/control"})) as binaries, \
             patch.object(gate, "beacon_fixture_root", return_value=Path("/private/beacon")) as private, \
             patch.object(gate, "compile_harness", return_value=FixtureCopies("/warm/network")), \
             patch.object(gate.tempfile, "mkdtemp", return_value="/warm/private-fixture") as fixture, \
             patch.object(gate, "run_stages") as run, contextlib.redirect_stdout(io.StringIO()):
            gate.run_network_checks(Path("/frozen"), Path("/warm"), env, (77, 88),
                                    harness="/warm/network", stages=gate.BASIC_NETWORK_STAGES)
        selected = run.call_args.args[2]
        for key in ("IROHA_TEST_SKIP_BUILD", "IROHA_FAIL_ON_SANDBOX_SKIP", "IROHA_TEST_REQUIRE_NETWORK", "IROHA_TEST_SERIALIZE_NETWORKS", "IROHA_TEST_NETWORK_KEEP_DIRS"):
            self.assertEqual(selected[key], "1")
        self.assertEqual(selected["TEST_NETWORK_BIN_IROHAD"], "/warm/node")
        self.assertEqual(selected["TEST_NETWORK_BIN_IROHAD_TAIRA"], "/warm/taira")
        self.assertEqual(selected["TEST_NETWORK_BIN_IROHA"], "/warm/client")
        self.assertEqual(selected["TEST_NETWORK_TMP_DIR"], "/warm/private-fixture")
        self.assertEqual(tuple(stage for call in run.call_args_list for stage in call.args[3]),
                         gate.BASIC_NETWORK_STAGES)
        self.assertTrue(all(call.args[4] == (77, 88) for call in run.call_args_list))
        self.assertEqual(fixture.call_args.kwargs["dir"], Path("/warm"))
        self.assertEqual([call.kwargs for call in binaries.call_args_list], [{}, {"message_control": True}])
        private.assert_called_once_with()
        for call in run.call_args_list:
            beacon = call.args[3] == gate.BEACON_NETWORK_STAGES
            self.assertEqual("KAGAMI_BIN" in call.args[2], beacon)
            self.assertEqual("TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL" in call.args[2], beacon)
            if beacon:
                self.assertEqual(call.args[2]["KAGAMI_BIN"], "/warm/kagami")
                self.assertEqual(call.args[2]["TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL"], "/warm/control")
                self.assertEqual(call.args[2]["TAIRA_TESTNET_BEACON_FIXTURE_DIR"], "/private/beacon")

    def test_beacon_fixture_missing_kagami_stops_without_build_fallback(self):
        with patch.object(gate, "compile_network_binaries", return_value=FixtureCopies({
                "iroha3d": "/node", "iroha": "/cli", "taira-launcher": "/taira"})) as binaries, \
             patch.object(gate.tempfile, "mkdtemp", return_value="/warm/fixture"), \
             patch.object(gate, "beacon_fixture_root") as custody, \
             patch.object(gate, "run_stages") as run, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "isolated shipping Kagami"):
                gate.run_network_checks(Path("/frozen"), Path("/warm"), {"CARGO_TARGET_DIR": "/warm"}, (),
                                        harness="/network", stages=gate.BEACON_NETWORK_STAGES)
        binaries.assert_called_once_with(Path("/frozen"), {"CARGO_TARGET_DIR": "/warm"}, ())
        custody.assert_not_called()
        run.assert_not_called()

    def test_beacon_fixture_custody_failure_stops_before_feature_build_or_execution(self):
        with patch.object(gate, "compile_network_binaries", return_value=FixtureCopies({
                "iroha3d": "/node", "iroha": "/cli", "taira-launcher": "/taira", "kagami": "/kagami"})) as binaries, \
             patch.object(gate.tempfile, "mkdtemp", return_value="/warm/fixture"), \
             patch.object(gate, "beacon_fixture_root", side_effect=gate.CheckError("untrusted fixture custody")), \
             patch.object(gate, "run_stages") as run, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "untrusted fixture custody"):
                gate.run_network_checks(Path("/frozen"), Path("/warm"), {"CARGO_TARGET_DIR": "/warm"}, (),
                                        harness="/network", stages=gate.BEACON_NETWORK_STAGES)
        binaries.assert_called_once_with(Path("/frozen"), {"CARGO_TARGET_DIR": "/warm"}, ())
        run.assert_not_called()

    def test_one_real_custody_case_preserves_catalog_and_both_route_sequences_in_each_scope(self):
        catalog = EXPECTED_BEACON_NETWORK_TEST
        for scope in gate.QUALIFICATION_SCOPES:
            with self.subTest(scope=scope):
                stages = gate.qualification_stages(scope)["network"]
                expected = [name for _, names in stages for name in names]
                observations = [name for _, names in gate.NETWORK_OBSERVATION_STAGES for name in names]
                self.assertEqual(expected[:len(observations)], observations)
                expensive = expected[len(observations):]
                self.assertEqual(expensive, [catalog])
                self.assertEqual(len(expected), len(set(expected)))
                seen = []
                def native(command, **kwargs):
                    if "--list" in command:
                        output = "".join(name + ": test\n" for name in expected)
                    else:
                        seen.append(command[1])
                        output = (f"test {command[1]} ... ok\n"
                                  "test result: ok. 1 passed; 0 failed; 0 ignored;\n")
                    return subprocess.CompletedProcess(command, 0, output, "")
                with patch.object(gate, "compile_network_binaries", return_value=FixtureCopies({
                        "iroha3d": "/node", "iroha": "/cli", "taira-launcher": "/taira", "kagami": "/kagami", "iroha3d-message-control": "/control"})), \
                     patch.object(gate, "beacon_fixture_root", return_value=Path("/private/beacon")), \
                     patch.object(gate.tempfile, "mkdtemp", return_value="/warm/fixture"), \
                     patch.object(gate.subprocess, "run", side_effect=native), \
                     contextlib.redirect_stdout(io.StringIO()):
                    gate.run_network_checks(Path("/frozen"), Path("/warm"), {"CARGO_TARGET_DIR": "/warm"}, (),
                                            harness="/network", stages=stages)
                self.assertEqual(seen, expected, "every selected test must execute exactly once")

    def test_failed_real_custody_case_is_reported_once_without_another_network_start(self):
        stages = gate.NETWORK_STAGES
        expected = [name for _, names in stages for name in names]
        catalog = EXPECTED_BEACON_NETWORK_TEST
        seen = []
        def native(command, **kwargs):
            if "--list" in command:
                return subprocess.CompletedProcess(command, 0, "".join(name + ": test\n" for name in expected), "")
            seen.append(command[1])
            if command[1] == catalog:
                return subprocess.CompletedProcess(command, 101, "exact cold recovery failed\n", "")
            return subprocess.CompletedProcess(command, 0,
                f"test {command[1]} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n", "")
        with patch.object(gate, "compile_network_binaries", return_value=FixtureCopies({
                "iroha3d": "/node", "iroha": "/cli", "taira-launcher": "/taira", "kagami": "/kagami", "iroha3d-message-control": "/control"})), \
             patch.object(gate, "beacon_fixture_root", return_value=Path("/private/beacon")), \
             patch.object(gate.tempfile, "mkdtemp", return_value="/warm/fixture"), \
             patch.object(gate.subprocess, "run", side_effect=native), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(gate.SelectedRegressionFailures) as failed:
                gate.run_network_checks(Path("/frozen"), Path("/warm"), {"CARGO_TARGET_DIR": "/warm"}, (),
                                        harness="/network", stages=stages)
        self.assertEqual(seen, expected[:expected.index(catalog) + 1])
        self.assertEqual(len(failed.exception.failures), 1)
        self.assertIn(catalog, failed.exception.failures[0])

    def test_network_observation_failures_aggregate_before_any_peer_case(self):
        expected = [name for _, names in gate.NETWORK_STAGES for name in names]
        observations = [name for _, names in gate.NETWORK_OBSERVATION_STAGES for name in names]
        seen = []
        def native(command, **kwargs):
            if "--list" in command:
                return subprocess.CompletedProcess(command, 0, "".join(name + ": test\n" for name in expected), "")
            seen.append(command[1])
            return subprocess.CompletedProcess(command, 101, "independent observation failed\n", "")
        with patch.object(gate, "compile_network_binaries", return_value=FixtureCopies({
                "iroha3d": "/node", "iroha": "/cli", "taira-launcher": "/taira"})), \
             patch.object(gate.tempfile, "mkdtemp", return_value="/warm/fixture"), \
             patch.object(gate.subprocess, "run", side_effect=native), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(gate.SelectedRegressionFailures) as failed:
                gate.run_network_checks(Path("/frozen"), Path("/warm"), {"CARGO_TARGET_DIR": "/warm"}, (),
                                        harness="/network", stages=gate.NETWORK_STAGES)
        self.assertEqual(seen, observations)
        self.assertEqual(len(failed.exception.failures), len(observations))


class FocusedNetworkObservationTests(unittest.TestCase):
    def test_one_observation_runs_without_shipping_capacity_or_peer_workspace(self):
        name = gate.NETWORK_OBSERVATION_STAGES[0][1][0]
        stages = gate.focused_regression_stages("basic", ("network=" + name,))["network"]
        env = {"CARGO_TARGET_DIR": "/warm"}
        executed = []

        def native(command, **kwargs):
            self.assertEqual(kwargs["env"], env)
            self.assertEqual(kwargs["cwd"], Path("/warm"))
            self.assertEqual(kwargs["pass_fds"], (77,))
            if "--list" in command:
                output = name + ": test\n"
            else:
                executed.append(command[1])
                output = f"test {name} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n"
            return subprocess.CompletedProcess(command, 0, output, "")

        with patch.object(gate, "compile_network_binaries") as build, \
             patch.object(gate, "require_network_fixture_capacity") as capacity, \
             patch.object(gate, "beacon_fixture_root") as custody, \
             patch.object(gate.tempfile, "mkdtemp") as workspace, \
             patch.object(gate.subprocess, "run", side_effect=native), \
             contextlib.redirect_stdout(io.StringIO()):
            gate.run_network_checks(Path("/source"), Path("/warm"), env, (77,),
                                    harness="/network", stages=stages)
        self.assertEqual(executed, [name])
        for prerequisite in (build, capacity, custody, workspace):
            prerequisite.assert_not_called()

    def test_partial_observation_groups_aggregate_before_shipping_and_beacon(self):
        names = (gate.NETWORK_OBSERVATION_STAGES[0][1][0],
                 gate.NETWORK_OBSERVATION_STAGES[2][1][0])
        stages = gate.focused_regression_stages("basic", tuple(
            "network=" + name for name in (*names, gate.BEACON_NETWORK_TEST)))["network"]
        executed = []

        def native(command, **kwargs):
            if "--list" in command:
                return subprocess.CompletedProcess(command, 0,
                    "".join(name + ": test\n" for name in (*names, gate.BEACON_NETWORK_TEST)), "")
            executed.append(command[1])
            return subprocess.CompletedProcess(command, 101, "observation failed\n", "")

        with patch.object(gate, "compile_network_binaries") as build, \
             patch.object(gate.tempfile, "mkdtemp") as workspace, \
             patch.object(gate.subprocess, "run", side_effect=native), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(gate.SelectedRegressionFailures) as failed:
                gate.run_network_checks(Path("/source"), Path("/warm"), {}, (),
                                        harness="/network", stages=stages)
        self.assertEqual(executed, list(names))
        self.assertEqual(len(failed.exception.failures), len(names))
        build.assert_not_called()
        workspace.assert_not_called()

    def test_selected_observations_complete_before_shipping_codegen(self):
        name = gate.NETWORK_OBSERVATION_STAGES[0][1][0]
        stages = gate.focused_regression_stages("basic", (
            "network=" + name, "network=" + gate.BEACON_NETWORK_TEST))["network"]
        order = []

        def observe(harness, directory, env, selected, locks):
            self.assertEqual([test for _, tests in selected for test in tests], [name])
            order.append("observation")

        def build(*args, **kwargs):
            order.append("shipping")
            raise gate.CheckError("shipping codegen failed")

        with patch.object(gate, "run_stages", side_effect=observe), \
             patch.object(gate, "compile_network_binaries", side_effect=build):
            with self.assertRaisesRegex(gate.CheckError, "shipping codegen failed"):
                gate.run_network_checks(Path("/source"), Path("/warm"), {}, (),
                                        harness="/network", stages=stages)
        self.assertEqual(order, ["observation", "shipping"])

    def test_partition_preserves_mixed_group_order_and_unknown_runtime_cases(self):
        first, second = gate.NETWORK_OBSERVATION_STAGES[0][1]
        selected = (("mixed", (first, "runtime-control", second)),
                    ("beacon", (gate.BEACON_NETWORK_TEST,)))
        observations, runtime = gate.split_network_stages(selected)
        self.assertEqual(observations, (("mixed", (first, second)),))
        self.assertEqual(runtime, (("mixed", ("runtime-control",)),
                                  ("beacon", (gate.BEACON_NETWORK_TEST,))))

    def test_empty_selection_does_not_compile_or_start_a_fixture(self):
        with patch.object(gate, "compile_network_binaries") as build, \
             patch.object(gate, "run_stages") as run:
            gate.run_network_checks(Path("/source"), Path("/warm"), {}, (),
                                    harness="/network", stages=())
        build.assert_not_called()
        run.assert_not_called()


class EarlyConfigurationGateTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)
        metadata = patch.object(gate, "check_test_harnesses")
        metadata.start()
        self.addCleanup(metadata.stop)

    env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated",
           "CARGO_TARGET_DIR": "/warm", "CARGO_INCREMENTAL": "1"}

    def test_configuration_target_preserves_defaults_and_requires_its_exact_artifact(self):
        self.assertEqual(gate.compile_command(Path("/frozen"), self.env, harness="config"), [
            "/fixed/cargo", "--config", "/frozen/.cargo/config.toml", "test",
            "--manifest-path", "/frozen/Cargo.toml", "--locked", "--offline",
            "-p", "iroha_config", "--test", "taira_config_contracts", "--no-run",
            "--message-format=json-render-diagnostics",
        ])
        event = {"reason": "compiler-artifact", "target": {
            "name": "taira_config_contracts", "kind": ["test"]},
            "profile": {"test": True}, "executable": "/warm/config-contracts"}
        self.assertEqual(gate.test_artifact(json.dumps(event), harness="config"), event["executable"])
        for changes in ({"profile": {"test": False}}, {"executable": None},
                        {"target": {"name": "iroha_config_integration", "kind": ["test"]}},
                        {"target": {"name": "taira_config_contracts", "kind": ["lib"]}}):
            self.assertIsNone(gate.test_artifact(json.dumps(event | changes), harness="config"))

    def test_configuration_stage_uses_same_captured_root_environment_and_locks(self):
        copies = FixtureCopies({"config": "/warm/config-contracts"})
        with patch.object(gate, "compile_harness") as compile, \
             patch.object(copies, "release") as release, \
             patch.object(gate, "run_stages") as run:
            gate.run_config_checks(copies, Path("/warm"), self.env, (77, 88))
        compile.assert_not_called()
        run.assert_called_once_with("/warm/config-contracts", Path("/warm"), self.env,
                                    gate.CONFIG_STAGES, (77, 88))
        release.assert_called_once_with("config")
        self.assertIs(run.call_args.args[2], self.env)

    def test_configuration_library_cannot_be_replaced_by_its_integration_harness(self):
        self.assertEqual(gate.compile_command(Path("/frozen"), self.env, harness="config-unit"), [
            "/fixed/cargo", "--config", "/frozen/.cargo/config.toml", "test",
            "--manifest-path", "/frozen/Cargo.toml", "--locked", "--offline",
            "-p", "iroha_config", "--lib", "--no-run", "--message-format=json-render-diagnostics",
        ])
        event = {"reason": "compiler-artifact", "target": {
            "name": "iroha_config", "kind": ["lib"]},
            "profile": {"test": True}, "executable": "/warm/config-unit"}
        self.assertEqual(gate.test_artifact(json.dumps(event), harness="config-unit"), event["executable"])
        self.assertIsNone(gate.test_artifact(json.dumps(event), harness="config"))
        for changes in ({"profile": {"test": False}}, {"executable": None},
                        {"target": {"name": "taira_config_contracts", "kind": ["test"]}},
                        {"target": {"name": "iroha_config", "kind": ["bin"]}}):
            with self.subTest(changes=changes):
                self.assertIsNone(gate.test_artifact(json.dumps(event | changes), harness="config-unit"))

    def test_one_batch_runs_configuration_first_after_source_audits(self):
        events = []
        libraries = ("config", "mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread", "config-unit", "data-model", "proof", "proof-flows", "crypto", "p2p", "core", "test-network", "client", "wallet", "torii-unit", "torii", "torii-shared", "torii-lifecycle", "daemon", "network", "cli")

        def run_stage(harness, root, env, stages, lock_fds, **kwargs):
            self.assertEqual(kwargs, {"batch": True} if harness == "/warm/cli" else {})
            self.assertEqual((root, lock_fds), (Path("/warm"), (77,)))
            events.append("config-pass" if stages == gate.CONFIG_STAGES else harness)

        def compile_libraries(*args, **kwargs):
            self.assertEqual(events, ["fsm", "source"])
            self.assertEqual(kwargs, {"lock_fds": (77,), "harnesses": libraries})
            events.append("library-build")
            return FixtureCopies({name: "/warm/" + name for name in libraries})

        with patch.object(gate, "require_network_fixture_capacity"), \
             patch.object(gate, "run_pure_fsm_checks", side_effect=lambda *args: events.append("fsm")), \
             patch.object(gate, "run_lifecycle_source_checks", side_effect=lambda *args: events.append("source")), \
             patch.object(gate, "compile_harness") as separate, \
             patch.object(gate, "compile_test_harnesses", side_effect=compile_libraries) as batch, \
             patch.object(gate, "run_stages", side_effect=run_stage), \
             patch.object(gate, "run_network_checks", side_effect=gate.CheckError("stop after ordering check")), \
             patch.object(gate.subprocess, "check_output", side_effect=AssertionError("captured source requires no Git lookup")), \
             contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "stop after ordering check"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=self.env,
                                source_commit="a" * 40, lock_fds=(77,))
        separate.assert_not_called()
        self.assertEqual(batch.call_count, 1)
        self.assertEqual(events, ["fsm", "source", "library-build", "config-pass",
                                  *["/warm/" + name for name in ("mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread", "core", "core", "torii-unit", "daemon", "cli") + libraries[6:-2]]])

    def test_batch_or_configuration_failure_stops_all_later_execution_and_passes(self):
        for phase in ("build", "schema"):
            output = io.StringIO()
            with self.subTest(phase=phase), \
                 patch.object(gate, "shipping_harnesses", return_value=("kagami",)), \
                 patch.object(gate, "require_network_fixture_capacity"), \
                 patch.object(gate, "run_pure_fsm_checks"), \
                 patch.object(gate, "run_lifecycle_source_checks"), \
                 patch.object(gate, "compile_harness") as compile, \
                 patch.object(gate, "run_stages", side_effect=gate.SelectedRegressionFailures(["config schema failed"])) as run, \
                patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies("/warm/config"),
                              side_effect=gate.CheckError("config build failed") if phase == "build" else None) as libraries, \
                 patch.object(gate, "check_shipping_binaries") as metadata, \
                 patch.object(gate, "run_network_checks") as network, \
                 contextlib.redirect_stdout(output):
                checkpoint = MagicMock()
                with self.assertRaisesRegex(gate.CheckError, "config " + phase + " failed"):
                    gate.run_checks(Path("/frozen"), qualification_scope="full", environment=self.env,
                                    source_commit="a" * 40, lock_fds=(77, 88),
                                    update_independent_checks=checkpoint)
            compile.assert_not_called()
            self.assertEqual(libraries.call_count, 1)
            if phase == "build":
                run.assert_not_called()
            else:
                self.assertEqual(run.call_count, 1)
                self.assertEqual(run.call_args.args[3:], (gate.CONFIG_STAGES, (77, 88)))
            checkpoint.assert_not_called()
            metadata.assert_not_called()
            network.assert_not_called()
            self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_configuration_always_runs_before_exact_independent_checkpoint_reuse(self):
        for config_fails in (False, True):
            with self.subTest(config_fails=config_fails), contextlib.ExitStack() as stack:
                for name in ("MV_OWNERSHIP_STAGES", "MV_EBR_STAGES", "MV_MAP_STAGES", "MV_ADMITTED_MAP_STAGES", "CONCREAD_STAGES",
                             "CONFIG_UNIT_STAGES", "DATA_MODEL_STAGES", "CRYPTO_STAGES", "P2P_STAGES", "CORE_STAGES", "TEST_NETWORK_STAGES",
                             "CLIENT_STAGES", "WALLET_STAGES", "TORII_UNIT_STAGES", "TORII_STAGES", "TORII_SHARED_STAGES", "TORII_LIFECYCLE_STAGES", "DAEMON_STAGES",
                             "PROOF_STAGES", "PROOF_FLOW_STAGES"):
                    stack.enter_context(patch.object(gate, name, ()))
                copies = FixtureCopies({name: "/warm/" + name for name in ("config", "cli", "network")})
                copies.observations = [{"selection": name, "sha256": str(index) * 64, "size": 20,
                                        "cargo_artifact": {"name": name}}
                                       for index, name in enumerate(copies, 1)]
                evidence = gate.independent_check_evidence(copies, (("cli", gate.STAGES),), qualification_scope="full")
                for name in ("run_pure_fsm_checks", "run_lifecycle_source_checks", "require_network_fixture_capacity"):
                    stack.enter_context(patch.object(gate, name))
                batch = stack.enter_context(patch.object(gate, "compile_test_harnesses", return_value=copies))
                separate = stack.enter_context(patch.object(gate, "compile_harness"))
                network = stack.enter_context(patch.object(gate, "run_network_checks"))
                run = stack.enter_context(patch.object(gate, "run_stages", side_effect=
                    gate.SelectedRegressionFailures(["config failed"]) if config_fails else None))
                released = stack.enter_context(patch.object(copies, "release"))
                output = stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
                checkpoint = MagicMock()
                arguments = dict(environment=self.env, source_commit="a" * 40,
                                 completed_independent_checks=evidence,
                                 update_independent_checks=checkpoint)
                if config_fails:
                    with self.assertRaisesRegex(gate.SelectedRegressionFailures, "config failed"):
                        gate.run_checks(Path("/frozen"), qualification_scope="full", **arguments)
                    network.assert_not_called()
                    self.assertNotIn("reused exact", output.getvalue())
                    self.assertNotIn("[taira-check] PASS:", output.getvalue())
                else:
                    gate.run_checks(Path("/frozen"), qualification_scope="full", **arguments)
                    network.assert_called_once()
                    self.assertIn("reused exact", output.getvalue())
                    self.assertEqual([call.args[0] for call in released.call_args_list], ["config", "cli", "network"])
                run.assert_called_once()
                self.assertEqual(run.call_args.args[3], gate.CONFIG_STAGES)
                self.assertEqual(batch.call_args.kwargs["harnesses"], ("config", "network", "cli"))
                checkpoint.assert_not_called()
                separate.assert_not_called()
        self.assertEqual(gate.selected_regression_count("full"), EXPECTED_REGRESSION_COUNT)


class NetworkFixtureCapacityTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self, keep_network_prerequisites=True)
        custody = patch.object(gate, "beacon_fixture_root")
        custody.start()
        self.addCleanup(custody.stop)

    def test_storage_floor_accepts_exact_boundary_and_rejects_one_byte_less(self):
        for available, passes in ((gate.NETWORK_FIXTURE_FREE_BYTES, True),
                                  (gate.NETWORK_FIXTURE_FREE_BYTES - 1, False)):
            with self.subTest(available=available), patch.object(
                    gate.shutil, "disk_usage", return_value=MagicMock(free=available)) as usage:
                if passes:
                    gate.require_network_fixture_capacity(Path("/warm"))
                else:
                    with self.assertRaisesRegex(gate.CheckError, "four-peer fixtures require"):
                        gate.require_network_fixture_capacity(Path("/warm"))
                usage.assert_called_once_with(Path("/warm"))

    def test_insufficient_space_stops_before_compilation(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        with patch.object(gate.shutil, "disk_usage", return_value=MagicMock(free=0)), \
             patch.object(gate, "compile_harness") as compile, \
             patch.object(gate, "run_pure_fsm_checks") as fsm, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "four-peer fixtures require"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40)
        compile.assert_not_called()
        fsm.assert_not_called()

    def test_capacity_is_checked_again_after_builds_before_starting_peers(self):
        with patch.object(gate, "compile_network_binaries", return_value=FixtureCopies({"iroha3d": "/node", "iroha": "/cli", "taira-launcher": "/taira"})), \
             patch.object(gate, "compile_harness", return_value=FixtureCopies("/harness")), \
             patch.object(gate.shutil, "disk_usage", return_value=MagicMock(free=0)), \
             patch.object(gate, "run_stages") as run, patch.object(gate.tempfile, "mkdtemp") as fixture:
            with self.assertRaisesRegex(gate.CheckError, "four-peer fixtures require"):
                gate.run_network_checks(Path("/frozen"), Path("/warm"), {"CARGO_TARGET_DIR": "/warm"}, (),
                                        harness="/harness", stages=gate.NETWORK_STAGES)
        run.assert_called_once_with("/harness", Path("/warm"), {"CARGO_TARGET_DIR": "/warm"},
                                    gate.NETWORK_OBSERVATION_STAGES, ())
        fixture.assert_not_called()


class CargoBuildProgressTests(unittest.TestCase):
    @staticmethod
    def event(name, kind="lib", *, test=True, fresh=True, features=None):
        return {"reason": "compiler-artifact", "package_id": f"path+file:///source#{name}@1",
                "target": {"name": name, "kind": [kind]}, "profile": {"test": test},
                "features": features or [], "filenames": [f"/warm/{name}"],
                "executable": f"/warm/{name}" if test or kind == "bin" else None, "fresh": fresh}

    @staticmethod
    def reports(output):
        return [json.loads(line.removeprefix("[taira-cargo] "))
                for line in output.getvalue().splitlines() if line.startswith("[taira-cargo] {")]

    def test_observed_extra_tests_and_cache_counts_do_not_inflate_on_duplicate_events(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            progress = gate.CargoBuildProgress("test codegen", {("lib", "iroha_core")}, test_profile=True)
            events = [self.event("iroha_core", fresh=False), self.event("iroha_core", fresh=False),
                      self.event("iroha_config"), self.event("iroha_test_network"),
                      self.event("dependency", test=False), self.event("unknown", test=False, fresh=None)]
            for event in events:
                progress.observe(json.dumps(event))
            progress.report("Cargo exited 0")
        report = self.reports(output)[-1]
        self.assertEqual(report["requested_targets"], ["lib:iroha_core"])
        self.assertEqual(report["observed_targets"], ["lib:iroha_config", "lib:iroha_core", "lib:iroha_test_network"])
        self.assertEqual(report["additional_targets"], ["lib:iroha_config", "lib:iroha_test_network"])
        self.assertEqual(report["observed_artifact_units"], {"fresh": 3, "rebuilt": 1, "unknown": 1})
        self.assertNotIn("PASS", output.getvalue())

    def test_distinct_feature_and_profile_units_stay_distinct_and_rebuild_wins_repeated_freshness(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            progress = gate.CargoBuildProgress("test metadata", {("lib", "model")}, test_profile=True)
            original = self.event("model", features=["test-fixtures"])
            for event in (original, original | {"fresh": False}, original,
                          self.event("model", features=["privacy-exact12-conformance"]),
                          self.event("model", test=False, features=["test-fixtures"]),
                          self.event("other", test=False, fresh=1)):
                progress.observe(json.dumps(event))
            progress.report("Cargo exited 101")
        report = self.reports(output)[-1]
        self.assertEqual(report["observed_artifact_units"], {"fresh": 2, "rebuilt": 1, "unknown": 1})
        self.assertEqual(report["state"], "Cargo exited 101")
        self.assertNotIn("PASS", output.getvalue())

    def test_diagnostics_and_nonartifact_events_are_not_work_and_running_output_is_throttled(self):
        output = io.StringIO()
        with patch.object(gate.time, "monotonic", return_value=100) as clock, contextlib.redirect_stdout(output):
            progress = gate.CargoBuildProgress("test codegen", {("lib", "core")}, test_profile=True)
            for line in ("compiler output\n", "[]", "null", '{"reason":"build-finished","success":false}',
                         '{"reason":"compiler-artifact","target":null}'):
                progress.observe(line)
            self.assertEqual(progress.units, {})
            progress.observe(json.dumps(self.event("dependency", test=False)))
            self.assertEqual(self.reports(output), [])
            clock.return_value = 130
            progress.observe(json.dumps(self.event("core")))
            self.assertEqual(len(self.reports(output)), 1)
            clock.return_value = 131
            progress.observe(json.dumps(self.event("extra")))
            self.assertEqual(len(self.reports(output)), 1)
            progress.report("Cargo exited 0")
        self.assertEqual([r["state"] for r in self.reports(output)], ["running", "Cargo exited 0"])
        self.assertEqual(self.reports(output)[-1]["elapsed_seconds"], 31)

    def test_shipping_target_observations_exclude_test_binary_and_build_script(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            progress = gate.CargoBuildProgress("shipping codegen", {("bin", "iroha3d")}, test_profile=False)
            for event in (self.event("iroha3d", "bin", test=False, fresh=False),
                          self.event("iroha", "bin", test=True),
                          self.event("build-script-build", "custom-build", test=False)):
                progress.observe(json.dumps(event))
            progress.report("Cargo exited 0")
        report = self.reports(output)[-1]
        self.assertEqual(report["observed_targets"], ["bin:iroha3d"])
        self.assertEqual(report["observed_artifact_units"], {"fresh": 2, "rebuilt": 1, "unknown": 0})


class CargoQuietBuildProgressTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)

    def test_every_cargo_phase_creates_private_outputs_with_permissive_parent_umask(self):
        real_popen = subprocess.Popen
        phases = (
            (lambda env: gate.check_test_harnesses(Path("/frozen"), env, harnesses=("config",)),
             [CargoBuildProgressTests.event("taira_config_contracts", "test")]),
            (lambda env: gate.compile_test_harnesses(Path("/frozen"), env, harnesses=("config",)),
             [CargoBuildProgressTests.event("taira_config_contracts", "test")]),
            (lambda env: gate.compile_network_binaries(Path("/frozen"), env, ()),
             [CargoBuildProgressTests.event(name, "bin", test=False) for name in ("iroha3d", "iroha", "iroha3d_taira")]),
        )
        for operation, events in phases:
            with self.subTest(operation=operation), tempfile.TemporaryDirectory() as temporary:
                target = Path(temporary).resolve()
                env = {"CARGO": "/unused", "CARGO_TARGET_DIR": str(target)}
                source = ("import os; "
                          f"os.mkdir({str(target / 'debug')!r}, 0o777); "
                          f"os.close(os.open({str(target / 'debug/.cargo-lock')!r}, os.O_CREAT|os.O_WRONLY, 0o666)); "
                          f"os.close(os.open({str(target / 'debug/program')!r}, os.O_CREAT|os.O_WRONLY, 0o777)); "
                          f"print({chr(10).join(json.dumps(event) for event in events)!r})")

                def cargo_child(_command, **kwargs):
                    return real_popen([sys.executable, "-c", source], **kwargs)

                original_umask = os.umask(0o002)
                try:
                    with patch.object(gate.subprocess, "Popen", side_effect=cargo_child), \
                         patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: rows), \
                         contextlib.redirect_stdout(io.StringIO()):
                        operation(env)
                    self.assertEqual(os.umask(0o002), 0o002)
                finally:
                    os.umask(original_umask)
                self.assertEqual(stat.S_IMODE((target / "debug").stat().st_mode), 0o700)
                self.assertEqual(stat.S_IMODE((target / "debug/.cargo-lock").stat().st_mode), 0o600)
                self.assertEqual(stat.S_IMODE((target / "debug/program").stat().st_mode), 0o700)
                with gate.native_artifact_guard(Path("/frozen"), target, env):
                    pass
                (target / "debug/.cargo-lock").chmod(0o664)
                with self.assertRaisesRegex(gate.CheckError, "unsafe native Cargo profile lock"):
                    with gate.native_artifact_guard(Path("/frozen"), target, env):
                        self.fail("unsafe existing lock was admitted")
                self.assertEqual(stat.S_IMODE((target / "debug/.cargo-lock").stat().st_mode), 0o664)

    def test_quiet_real_children_report_before_output_and_preserve_all_phase_outcomes(self):
        real_popen = subprocess.Popen
        phases = (
            ("test metadata", lambda: gate.check_test_harnesses(Path("/frozen"), {"CARGO": "/unused"}, harnesses=("config",)),
             [CargoBuildProgressTests.event("taira_config_contracts", "test")]),
            ("test codegen", lambda: gate.compile_test_harnesses(Path("/frozen"), {"CARGO": "/unused"}, harnesses=("config",)),
             [CargoBuildProgressTests.event("taira_config_contracts", "test")]),
            ("shipping codegen", lambda: gate.compile_network_binaries(Path("/frozen"), {"CARGO": "/unused"}, ()),
             [CargoBuildProgressTests.event(name, "bin", test=False) for name in ("iroha3d", "iroha", "iroha3d_taira")]),
        )
        for phase, operation, events in phases:
            for code in (0, 101):
                with self.subTest(phase=phase, code=code):
                    read_fd, release_fd = os.pipe()

                    class ReleaseOnProgress(io.StringIO):
                        released = False

                        def write(self, text):
                            count = super().write(text)
                            if not self.released and text.startswith("[taira-cargo] {"):
                                report = json.loads(text.removeprefix("[taira-cargo] "))
                                if report["state"] == "running":
                                    self.released = True
                                    os.write(release_fd, b"x")
                            return count

                    output = ReleaseOnProgress()
                    # The child stays silent until the progress reporter releases
                    # it. A bounded child deadline makes the old event-only runner
                    # fail this test instead of hanging it.
                    source = ("import os,select,sys; "
                              f"ready=select.select([{read_fd}],[],[],3)[0]; "
                              "sys.exit(98) if not ready else None; "
                              f"os.read({read_fd},1); "
                              f"print({chr(10).join(json.dumps(event) for event in events)!r}); "
                              f"sys.exit({code})")

                    def quiet_child(_command, **kwargs):
                        kwargs["pass_fds"] = (*kwargs["pass_fds"], read_fd)
                        return real_popen([sys.executable, "-c", source], **kwargs)

                    try:
                        with patch.object(gate, "CARGO_PROGRESS_INTERVAL_SECONDS", 0.01), \
                             patch.object(gate.subprocess, "Popen", side_effect=quiet_child), \
                             patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: rows) as isolate, \
                             contextlib.redirect_stdout(output):
                            if code:
                                with self.assertRaisesRegex(gate.CheckError, "exit 101"):
                                    operation()
                                isolate.assert_not_called()
                            else:
                                operation()
                        self.assertTrue(output.released)
                        reports = CargoBuildProgressTests.reports(output)
                        self.assertEqual(reports[0]["phase"], phase)
                        self.assertEqual(reports[0]["state"], "running")
                        self.assertEqual(reports[0]["observed_artifact_units"], {"fresh": 0, "rebuilt": 0, "unknown": 0})
                        self.assertEqual(reports[-1]["state"], f"Cargo exited {code}")
                    finally:
                        os.close(read_fd)
                        os.close(release_fd)


class NativeTestBatchBuildTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)

    names = ("crypto", "p2p", "core", "test-network")
    env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}

    @staticmethod
    def artifact(name, executable=None):
        return json.dumps({"reason": "compiler-artifact", "target": {
            "name": gate.HARNESS_TARGETS[name][1], "kind": [gate.HARNESS_TARGETS[name][2]]}, "profile": {"test": True},
            "executable": executable or "/warm/" + name}) + "\n"

    def process(self, lines, code=0):
        child = MagicMock()
        child.stdout = io.StringIO(lines)
        child.wait.return_value = code
        process = MagicMock()
        process.__enter__.return_value = child
        return process

    def test_resource_batch_requires_both_libraries_and_all_explicit_integration_artifacts(self):
        names = ("mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread")
        self.assertEqual(gate.native_harness_selection(names), [
            "-p", "mv", "-p", "concread", "--lib", "--test", "ebr_allocation_custody", "--test", "map_owned_generations", "--test", "admitted_map_custody",
        ])
        lines = "".join(self.artifact(name) for name in names)
        with patch.object(gate.subprocess, "Popen", return_value=self.process(lines)) as spawn, \
             patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: rows), \
             contextlib.redirect_stdout(io.StringIO()):
            actual = gate.compile_test_harnesses(Path("/frozen"), self.env,
                                                harnesses=names, lock_fds=(77,))
        self.assertEqual(set(actual), set(names))
        self.assertEqual(spawn.call_args.args[0], [
            "/fixed/cargo", "--config", "/frozen/.cargo/config.toml", "test",
            "--manifest-path", "/frozen/Cargo.toml", "--locked", "--offline",
            "-p", "mv", "-p", "concread", "--lib", "--test", "ebr_allocation_custody", "--test", "map_owned_generations", "--test", "admitted_map_custody",
            "--no-run", "--message-format=json-render-diagnostics",
        ])
        for missing in names:
            incomplete = "".join(self.artifact(name) for name in names if name != missing)
            with self.subTest(missing=missing), \
                 patch.object(gate.subprocess, "Popen", return_value=self.process(incomplete)), \
                 patch.object(gate, "isolate_native_artifacts") as isolate, \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "0 test executables"):
                    gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=names)
            isolate.assert_not_called()

    def test_one_build_preserves_captured_cargo_custody_and_complete_selected_artifacts(self):
        lines = "[cargo-fast] warm lane\n" + "".join(self.artifact(name) for name in reversed(self.names))
        lines += self.artifact("crypto")  # Repeating the same exact artifact is harmless.
        lines += json.dumps({"reason": "compiler-artifact", "target": {"name": "unrelated", "kind": ["lib"]},
                             "profile": {"test": True}, "executable": "/warm/unrelated"}) + "\n"
        with patch.object(gate.subprocess, "Popen", return_value=self.process(lines)) as spawn, \
             patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: {name: row["executable"] for name, row in rows.items()}), \
             contextlib.redirect_stdout(io.StringIO()):
            actual = gate.compile_test_harnesses(Path("/frozen"), self.env,
                                                    harnesses=self.names, lock_fds=(77, 88))
        self.assertEqual(actual, {name: "/warm/" + name for name in self.names})
        self.assertEqual(spawn.call_count, 1)
        self.assertEqual(spawn.call_args.args[0], ["/fixed/cargo", "--config", "/frozen/.cargo/config.toml",
            "test", "--manifest-path", "/frozen/Cargo.toml", "--locked", "--offline",
            "-p", "iroha_crypto", "-p", "iroha_p2p", "-p", "iroha_core", "-p", "iroha_test_network",
            "--lib", "--no-run", "--message-format=json-render-diagnostics"])
        self.assertEqual(spawn.call_args.kwargs["cwd"], "/")
        self.assertIs(spawn.call_args.kwargs["env"], self.env)
        self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77, 88))

    def test_mixed_batch_includes_configuration_in_one_graph_and_requires_every_artifact(self):
        names = ("config", "config-unit", "data-model", "proof", "proof-flows", "crypto", "p2p", "core", "test-network", "client", "wallet", "torii-unit", "torii", "torii-shared", "torii-lifecycle", "network")
        lines = "".join(self.artifact(name) for name in reversed(names))
        with patch.object(gate.subprocess, "Popen", return_value=self.process(lines)) as spawn, \
             patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: {name: row["executable"] for name, row in rows.items()}), \
             contextlib.redirect_stdout(io.StringIO()):
            actual = gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=names, lock_fds=(77,))
        self.assertEqual(set(actual), set(names))
        self.assertEqual(spawn.call_count, 1)
        self.assertEqual(spawn.call_args.args[0], ["/fixed/cargo", "--config", "/frozen/.cargo/config.toml",
            "test", "--manifest-path", "/frozen/Cargo.toml", "--locked", "--offline",
        "-p", "iroha_config", "-p", "iroha_data_model", "-p", "fastpq_prover", "-p", "iroha_crypto", "-p", "iroha_p2p", "-p", "iroha_core", "-p", "iroha_test_network",
            "-p", "iroha", "-p", "iroha_wallet", "-p", "iroha_torii", "-p", "iroha_torii_shared", "--test", "taira_config_contracts", "--lib", "--test", "fastpq_integration", "--test", "taira_app_contracts",
            "--test", "torii_nexus_sorafs", "--test", "taira_consensus_contracts", "--no-run", "--message-format=json-render-diagnostics"])
        for missing in ("config", "torii", "torii-shared", "torii-lifecycle", "network"):
            incomplete = "".join(self.artifact(name) for name in names if name != missing)
            with self.subTest(missing=missing), \
                 patch.object(gate.subprocess, "Popen", return_value=self.process(incomplete)), \
                 patch.object(gate, "isolate_native_artifacts") as isolate, \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "0 test executables"):
                    gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=names)
            isolate.assert_not_called()

    def test_invalid_selections_fail_before_cargo(self):
        for names in ((), ("crypto", "crypto"), ("unreviewed",), ("iroha",)):
            with self.subTest(names=names), patch.object(gate.subprocess, "Popen") as spawn:
                with self.assertRaises(gate.CheckError):
                    gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=names)
                spawn.assert_not_called()

    def test_metadata_and_codegen_report_implicit_targets_without_selecting_their_artifacts(self):
        names = ("config", "core", "network")
        events = [CargoBuildProgressTests.event(gate.HARNESS_TARGETS[name][1],
                  gate.HARNESS_TARGETS[name][2], fresh=False) for name in names]
        events += [CargoBuildProgressTests.event(name) for name in ("iroha_config", "iroha_test_network")]
        lines = "\n".join(json.dumps(event) for event in events)
        for operation, phase in ((gate.check_test_harnesses, "test metadata"),
                                 (gate.compile_test_harnesses, "test codegen")):
            output = io.StringIO()
            with self.subTest(phase=phase), patch.object(gate.subprocess, "Popen", return_value=self.process(lines)), \
                 patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: rows), \
                 contextlib.redirect_stdout(output):
                result = operation(Path("/frozen"), self.env, harnesses=names)
            if result is not None:
                self.assertEqual(set(result), set(names))
            report = CargoBuildProgressTests.reports(output)[-1]
            self.assertEqual(report["phase"], phase)
            self.assertEqual(len(report["requested_targets"]), 3)
            self.assertEqual(len(report["observed_targets"]), 5)
            self.assertEqual(report["additional_targets"], ["lib:iroha_config", "lib:iroha_test_network"])
            self.assertEqual(report["observed_artifact_units"], {"fresh": 2, "rebuilt": 3, "unknown": 0})

    def test_shipping_failure_still_reports_work_without_returning_test_artifacts(self):
        events = [CargoBuildProgressTests.event(name, "bin", test=False, fresh=False)
                  for name in ("iroha3d", "iroha", "iroha3d_taira")]
        events += [CargoBuildProgressTests.event("unexpected", "bin", test=True)]
        output = io.StringIO()
        with patch.object(gate.subprocess, "Popen", return_value=self.process(
                "\n".join(json.dumps(event) for event in events), 101)), \
             patch.object(gate, "isolate_native_artifacts") as isolate, contextlib.redirect_stdout(output):
            with self.assertRaisesRegex(gate.CheckError, "exit 101"):
                gate.compile_network_binaries(Path("/frozen"), self.env, ())
        isolate.assert_not_called()
        report = CargoBuildProgressTests.reports(output)[-1]
        self.assertEqual(report["phase"], "shipping codegen")
        self.assertEqual(report["observed_targets"], ["bin:iroha", "bin:iroha3d", "bin:iroha3d_taira"])
        self.assertEqual(report["state"], "Cargo exited 101")

    def test_incomplete_ambiguous_wrong_profile_or_shared_artifacts_fail_closed(self):
        good = "".join(self.artifact(name) for name in self.names)
        wrong_profile = json.loads(self.artifact("test-network"))
        wrong_profile["profile"]["test"] = False
        wrong_kind = json.loads(self.artifact("test-network"))
        wrong_kind["target"]["kind"] = ["bin"]
        without_fixture = "".join(self.artifact(name) for name in self.names[:-1])
        for lines, error in ((without_fixture, "0 test executables"),
                             (without_fixture + json.dumps(wrong_profile) + "\n", "0 test executables"),
                             (without_fixture + json.dumps(wrong_kind) + "\n", "0 test executables"),
                             (good + self.artifact("crypto", "/warm/other"), "2 test executables"),
                             ("".join(self.artifact(name, "/warm/same") for name in self.names), "reused one executable")):
            with self.subTest(error=error), \
                 patch.object(gate.subprocess, "Popen", return_value=self.process(lines)), \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, error):
                    gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=self.names)

    def test_cargo_failure_with_complete_artifacts_still_fails_and_preserves_diagnostic(self):
        diagnostic = "error[E0308]: synthetic library build failure\n"
        lines = "".join(self.artifact(name) for name in self.names)
        lines += json.dumps({"reason": "compiler-message", "message": {"rendered": diagnostic}}) + "\n"
        stderr = io.StringIO()
        with patch.object(gate.subprocess, "Popen", return_value=self.process(lines, 101)), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(stderr):
            with self.assertRaisesRegex(gate.CheckError, "build failed"):
                gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=self.names)
        self.assertEqual(stderr.getvalue(), diagnostic)

    def test_failed_batch_stops_before_any_native_test_or_network_start(self):
        with patch.object(gate, "run_pure_fsm_checks"), patch.object(gate, "run_lifecycle_source_checks"), \
             patch.object(gate, "check_test_harnesses"), \
             patch.object(gate, "run_config_checks"), \
             patch.object(gate, "require_network_fixture_capacity"), \
             patch.object(gate, "compile_test_harnesses", side_effect=gate.CheckError("batch failed")), \
             patch.object(gate, "run_stages") as run, patch.object(gate, "run_network_checks") as network, \
             patch.object(gate, "compile_harness") as other, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "batch failed"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=self.env, source_commit="a" * 40)
        run.assert_not_called()
        network.assert_not_called()
        other.assert_not_called()



class NativeTestMetadataCheckTests(unittest.TestCase):
    env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm",
           "CARGO_MAKEFLAGS": "--jobserver-auth=77,88", "RUSTFLAGS": "-C debuginfo=0"}
    process = NativeTestBatchBuildTests.process

    @staticmethod
    def artifact(name):
        return json.dumps({"reason": "compiler-artifact", "target": {
            "name": gate.HARNESS_TARGETS[name][1], "kind": [gate.HARNESS_TARGETS[name][2]]},
            "profile": {"test": True}, "executable": None,
            "filenames": ["/warm/metadata.rmeta"]}) + "\n"

    def test_metadata_checks_exact_complete_test_graph_with_preserved_cargo_custody(self):
        shipping = ("cli", "kagami", "taira-launcher", "sorafs-bin")
        for scope in gate.QUALIFICATION_SCOPES:
            _, names, _ = gate.native_harness_plan(gate.qualification_stages(scope), shipping)
            self.assertEqual(names, (
                "config", "mv", "mv-ebr", "mv-map", "mv-admitted-map", "concread", "config-unit", "data-model",
                "kagami", "proof", "proof-flows", "crypto", "p2p", "core",
                "test-network", "client", "wallet", "torii-unit", "torii", "torii-shared",
                "torii-lifecycle", "daemon", "network", "cli", "taira-launcher",
                "sorafs-bin",
            ))
            lines = "[cargo-fast] warm lane\n" + "".join(self.artifact(name) for name in names)
            lines += self.artifact("core")  # Cargo may repeat an identical target.
            output = io.StringIO()
            with self.subTest(scope=scope), \
                 patch.object(gate.subprocess, "Popen", return_value=self.process(lines)) as spawn, \
                 patch.object(gate, "_build_harnesses") as build, \
                 patch.object(gate, "isolate_native_artifacts") as isolate, contextlib.redirect_stdout(output):
                self.assertIsNone(gate.check_test_harnesses(Path("/frozen"), self.env,
                                                          harnesses=names, lock_fds=(77, 88)))
                gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=names, lock_fds=(77, 88))
            check_command = spawn.call_args.args[0]
            build_command = build.call_args.args[1]
            self.assertEqual(check_command[:3], build_command[:3])
            self.assertEqual(check_command[3], "check")
            self.assertEqual(build_command[3], "test")
            self.assertEqual(check_command[4:-3], build_command[4:-2])
            self.assertEqual(check_command[-3:], ["--profile", "test", "--message-format=json-render-diagnostics"])
            self.assertEqual(build_command[-2:], ["--no-run", "--message-format=json-render-diagnostics"])
            for forbidden in ("--tests", "--all-targets", "--all-features", "--no-default-features",
                              "--jobs", "-j", "--target-dir", "--keep-going", "-Z"):
                self.assertNotIn(forbidden, check_command)
            self.assertEqual(spawn.call_count, 1)
            self.assertEqual(spawn.call_args.kwargs["cwd"], "/")
            self.assertEqual(spawn.call_args.kwargs["stdin"], subprocess.DEVNULL)
            self.assertIs(spawn.call_args.kwargs["env"], self.env)
            self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77, 88))
            isolate.assert_not_called()
            self.assertIn("full harness compilation remains required", output.getvalue())
            self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_metadata_requires_every_selected_test_target_not_normal_library_artifacts(self):
        names = ("core", "cli", "network")
        core = json.loads(self.artifact("core"))
        wrong_profile = dict(core, profile={"test": False})
        wrong_kind = dict(core, target={"name": "iroha_core", "kind": ["bin"]})
        partial = self.artifact("cli") + self.artifact("network")
        for missing in ("", json.dumps(wrong_profile) + "\n", json.dumps(wrong_kind) + "\n"):
            with self.subTest(missing=missing), \
                 patch.object(gate.subprocess, "Popen", return_value=self.process(partial + missing)), \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "omitted selected test targets: core"):
                    gate.check_test_harnesses(Path("/frozen"), self.env, harnesses=names)

    def test_metadata_failure_keeps_compiler_diagnostic_even_with_all_artifact_events(self):
        diagnostic = "error[E0432]: synthetic unresolved test import\n"
        lines = self.artifact("network") + json.dumps({"reason": "compiler-message",
            "message": {"rendered": diagnostic}}) + "\n"
        error, output = io.StringIO(), io.StringIO()
        with patch.object(gate.subprocess, "Popen", return_value=self.process(lines, 101)), \
             contextlib.redirect_stdout(output), contextlib.redirect_stderr(error):
            with self.assertRaisesRegex(gate.CheckError, r"metadata check failed \(exit 101"):
                gate.check_test_harnesses(Path("/frozen"), self.env, harnesses=("network",))
        self.assertEqual(error.getvalue(), diagnostic)
        self.assertNotIn("metadata check passed", output.getvalue())

    def test_metadata_rejects_invalid_or_broadened_selections_before_cargo(self):
        for names in ((), ("core", "core"), ("unreviewed",)):
            with self.subTest(names=names), patch.object(gate.subprocess, "Popen") as spawn:
                with self.assertRaises(gate.CheckError):
                    gate.check_test_harnesses(Path("/frozen"), self.env, harnesses=names)
                spawn.assert_not_called()
        changed = (*gate.HARNESS_TARGETS["core"][:3], ["-p", "iroha_core", "--tests"])
        with patch.dict(gate.HARNESS_TARGETS, {"core": changed}), \
             patch.object(gate.subprocess, "Popen") as spawn:
            with self.assertRaisesRegex(gate.CheckError, "explicit library"):
                gate.check_test_harnesses(Path("/frozen"), self.env, harnesses=("core",))
            spawn.assert_not_called()


class ShippingMetadataCheckTests(unittest.TestCase):
    env = NativeTestMetadataCheckTests.env
    process = NativeTestBatchBuildTests.process
    shipping = ("taira-launcher", "cli", "sorafs-bin", "kagami")

    @staticmethod
    def artifact(name, *, test=False, kind="bin"):
        return json.dumps({"reason": "compiler-artifact", "target": {
            "name": gate.HARNESS_TARGETS[name][1], "kind": [kind]},
            "profile": {"test": test}, "executable": None,
            "filenames": ["/warm/production.rmeta"]}) + "\n"

    def test_shipping_metadata_uses_authoritative_default_feature_binaries_and_warm_custody(self):
        root = SCRIPT.parent.parent
        self.assertEqual(gate.shipping_harnesses(root), self.shipping)
        lines = "[cargo-fast] existing warm target\n" + "".join(self.artifact(name) for name in self.shipping)
        lines += self.artifact("cli")
        output = io.StringIO()
        with patch.object(gate.subprocess, "Popen", return_value=self.process(lines)) as spawn, \
             patch.object(gate, "isolate_native_artifacts") as isolate, contextlib.redirect_stdout(output):
            self.assertIsNone(gate.check_shipping_binaries(root, self.env, (77, 88)))
        self.assertEqual(spawn.call_args.args[0], [
            "/fixed/cargo", "--config", str(root / ".cargo/config.toml"), "check",
            "--manifest-path", str(root / "Cargo.toml"), "--locked", "--offline",
            "-p", "irohad", "-p", "iroha_cli", "-p", "sorafs_node", "-p", "iroha_kagami",
            "--bin", "iroha3d_taira", "--bin", "iroha", "--bin", "sorafs-node", "--bin", "kagami",
            "--message-format=json-render-diagnostics",
        ])
        for forbidden in ("--profile", "--tests", "--test", "--lib", "--all-targets", "--features",
                          "--all-features", "--no-default-features", "--target-dir", "--jobs", "-j"):
            self.assertNotIn(forbidden, spawn.call_args.args[0])
        self.assertEqual(spawn.call_args.kwargs["cwd"], "/")
        self.assertIs(spawn.call_args.kwargs["env"], self.env)
        self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77, 88))
        self.assertEqual(spawn.call_args.kwargs["stdin"], subprocess.DEVNULL)
        self.assertEqual(spawn.call_args.kwargs["umask"], 0o077)
        isolate.assert_not_called()
        report = CargoBuildProgressTests.reports(output)[-1]
        self.assertEqual(report["phase"], "shipping metadata")
        self.assertEqual(report["observed_targets"], report["requested_targets"])
        self.assertIn("shipping codegen and network qualification remain required", output.getvalue())
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_shipping_metadata_requires_every_production_binary_not_test_or_library_artifacts(self):
        partial = "".join(self.artifact(name) for name in self.shipping if name != "kagami")
        for missing in ("", self.artifact("kagami", test=True), self.artifact("kagami", kind="lib")):
            with self.subTest(missing=missing), \
                 patch.object(gate, "shipping_harnesses", return_value=self.shipping), \
                 patch.object(gate.subprocess, "Popen", return_value=self.process(partial + missing)), \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "omitted production binary targets: kagami"):
                    gate.check_shipping_binaries(Path("/frozen"), self.env, ())

    def test_shipping_metadata_failure_retains_diagnostics_and_never_claims_success(self):
        diagnostic = "error[E0599]: production method requires an unselected fixture feature\n"
        lines = "".join(self.artifact(name) for name in self.shipping)
        lines += json.dumps({"reason": "compiler-message", "message": {"rendered": diagnostic}}) + "\n"
        output, errors = io.StringIO(), io.StringIO()
        with patch.object(gate, "shipping_harnesses", return_value=self.shipping), \
             patch.object(gate.subprocess, "Popen", return_value=self.process(lines, 101)), \
             contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
            with self.assertRaisesRegex(gate.CheckError, r"shipping metadata check failed \(exit 101"):
                gate.check_shipping_binaries(Path("/frozen"), self.env, ())
        self.assertEqual(errors.getvalue(), diagnostic)
        self.assertNotIn("shipping metadata check passed", output.getvalue())
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_shipping_metadata_refuses_core_and_torii_fixture_feature_leaks(self):
        for library, feature in (("iroha_core", "iroha-core-tests"), ("iroha_torii", "test-fixtures")):
            event = {"reason": "compiler-artifact", "package_id": "opaque-cargo-package-identity",
                     "target": {"name": library, "kind": ["lib"]}, "profile": {"test": False},
                     "features": ["default", feature]}
            lines = json.dumps(event) + "\n" + "".join(self.artifact(name) for name in self.shipping)
            output = io.StringIO()
            with self.subTest(library=library), \
                 patch.object(gate, "shipping_harnesses", return_value=self.shipping), \
                 patch.object(gate.subprocess, "Popen", return_value=self.process(lines)), \
                 contextlib.redirect_stdout(output):
                with self.assertRaisesRegex(gate.CheckError, "forbidden fixture feature: " + library + "/" + feature):
                    gate.check_shipping_binaries(Path("/frozen"), self.env, ())
            self.assertNotIn("shipping metadata check passed", output.getvalue())

    def test_shipping_metadata_accepts_ordinary_features_and_matches_exact_library_targets(self):
        events = [{"reason": "compiler-artifact", "target": {"name": name, "kind": [kind]},
                   "profile": {"test": False}, "features": features}
                  for name, kind, features in (
                      ("iroha_core", "lib", ["default", "json"]),
                      ("iroha_torii", "lib", ["default", "app_api"]),
                      ("another_library", "lib", ["test-fixtures", "iroha-core-tests"]),
                      ("iroha_torii", "bin", ["test-fixtures"]),
                  )]
        lines = "".join(json.dumps(event) + "\n" for event in events)
        lines += "".join(self.artifact(name) for name in self.shipping)
        with patch.object(gate, "shipping_harnesses", return_value=self.shipping), \
             patch.object(gate.subprocess, "Popen", return_value=self.process(lines)), \
             contextlib.redirect_stdout(io.StringIO()):
            gate.check_shipping_binaries(Path("/frozen"), self.env, ())

    def test_shipping_metadata_requires_typed_feature_lists_for_protected_libraries(self):
        for features in (None, "default", ["default", None]):
            event = {"reason": "compiler-artifact", "target": {"name": "iroha_core", "kind": ["lib"]},
                     "profile": {"test": False}, "features": features}
            with self.subTest(features=features), \
                 patch.object(gate, "shipping_harnesses", return_value=self.shipping), \
                 patch.object(gate.subprocess, "Popen", return_value=self.process(json.dumps(event) + "\n")), \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "omitted production library features: iroha_core"):
                    gate.check_shipping_binaries(Path("/frozen"), self.env, ())

    def test_shipping_metadata_rejects_broadened_target_or_feature_injection_before_cargo(self):
        for names in ((), ("core",), ("config",), ("cli", "cli")):
            with self.subTest(names=names), patch.object(gate, "shipping_harnesses", return_value=names), \
                 patch.object(gate.subprocess, "Popen") as spawn:
                with self.assertRaises(gate.CheckError):
                    gate.check_shipping_binaries(Path("/frozen"), self.env, ())
                spawn.assert_not_called()
        injected = (*gate.HARNESS_TARGETS["cli"][:3], ["-p", "iroha_cli", "--bin", "iroha", "--features", "test-fixtures"])
        with patch.dict(gate.HARNESS_TARGETS, {"cli": injected}), \
             patch.object(gate, "shipping_harnesses", return_value=("cli",)), \
             patch.object(gate.subprocess, "Popen") as spawn:
            with self.assertRaisesRegex(gate.CheckError, "explicit library, integration or binary targets"):
                gate.check_shipping_binaries(Path("/frozen"), self.env, ())
            spawn.assert_not_called()


class NativeArtifactIsolationTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name).resolve()
        self.target = self.directory / "warm"
        self.source = self.target / "taira-release-sources" / "fixture" / "source"
        for directory in (self.target, self.target / "taira-release-sources",
                          self.source.parent, self.source):
            directory.mkdir(mode=0o700)
        (self.target / "debug").mkdir(mode=0o700)
        self.env = {"CARGO": "/fixed/cargo", "CARGO_TARGET_DIR": str(self.target)}
        import taira_cargo_cache as cache
        import release_artifact_contract as contract
        self.cache, self.contract = cache, contract
        metadata = patch.object(cache, "local_package_names", return_value={"irohad", "iroha_cli"})
        self.metadata = metadata.start()
        self.addCleanup(metadata.stop)
        self.stdout = io.StringIO()
        redirect = contextlib.redirect_stdout(self.stdout)
        redirect.__enter__()
        self.addCleanup(redirect.__exit__, None, None, None)
        closed = patch.object(gate, "native_test_output_confirmed_closed", return_value=True)
        self.closed = closed.start()
        self.addCleanup(closed.stop)
        # Retained read-only copies still belong to these disposable fixtures.
        self.addCleanup(self.make_fixture_writable)

    def make_fixture_writable(self):
        for path in self.target.glob("taira-native-artifacts-*"):
            path.chmod(0o700)
            for child in path.iterdir():
                child.chmod(0o600)

    def artifact(self, selection="iroha", payload=b"#!/bin/sh\nexit 0\n", *, shipping=False):
        if selection in ("iroha", "iroha3d") or shipping:
            package = "iroha_cli" if selection == "iroha" else "irohad"
            name = "iroha3d_taira" if selection == "taira-launcher" else selection
            kind, is_test = "bin", False
            executable = self.target / "debug" / name
        else:
            _, name, kind, arguments = gate.HARNESS_TARGETS[selection]
            package, is_test = arguments[1], True
            executable = self.target / "debug" / "deps" / (name + "-" + hashlib.sha256(selection.encode()).hexdigest()[:16])
            executable.parent.mkdir(mode=0o700, exist_ok=True)
        executable.write_bytes(payload)
        executable.chmod(0o700)
        row = {"name": name, "executable": str(executable), "profile": {"test": is_test},
               "manifest_path": str(self.source / "crates" / package / "Cargo.toml")}
        event = {"reason": "compiler-artifact", "target": {"name": name, "kind": [kind]},
                 **{key: value for key, value in row.items() if key != "name"}}
        return executable, row, event

    def assert_profile_locked(self):
        fd = os.open(self.target / "debug" / ".cargo-lock", os.O_RDWR)
        try:
            with self.assertRaises(BlockingIOError):
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        finally:
            os.close(fd)

    def assert_profile_unlocked(self):
        fd = os.open(self.target / "debug" / ".cargo-lock", os.O_RDWR)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        finally:
            os.close(fd)

    def isolate(self, rows):
        return gate.isolate_native_artifacts(self.source, self.env, rows)

    def test_cargo_publication_pair_is_copied_under_lock_without_changing_aliases(self):
        import taira_cargo_artifact as cargo
        executable, row, _ = self.artifact()
        deps = executable.parent / 'deps'; deps.mkdir(mode=0o700)
        alias = deps / (executable.name + '-0123456789abcdef')
        os.link(executable, alias)
        before = self.contract._open_anchored_regular
        def guarded(*args, **kwargs):
            self.assert_profile_locked()
            return before(*args, **kwargs)
        with patch.object(self.contract, '_open_anchored_regular', side_effect=guarded):
            copied = self.isolate({'iroha': row})
        destination = Path(copied['iroha'])
        self.assertEqual(destination.read_bytes(), executable.read_bytes())
        self.assertEqual(destination.stat().st_nlink, 1)
        self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o500)
        self.assertEqual(executable.stat().st_nlink, 2)
        self.assertEqual(executable.stat().st_ino, alias.stat().st_ino)
        self.assert_profile_unlocked()

    def test_real_profile_lock_covers_validation_and_copy_then_releases(self):
        executable, row, _ = self.artifact()
        original = self.contract.stable_hash_path
        def guarded_hash(*args, **kwargs):
            self.assert_profile_locked()
            return original(*args, **kwargs)
        with patch.object(self.contract, "stable_hash_path", side_effect=guarded_hash):
            actual = self.isolate({"iroha": row})
        self.assert_profile_unlocked()
        self.metadata.assert_called_once_with(self.source, self.env)
        copied = Path(actual["iroha"])
        self.assertNotEqual(executable.stat().st_ino, copied.stat().st_ino)
        self.assertEqual(copied.stat().st_nlink, 1)
        self.assertEqual(stat.S_IMODE(copied.stat().st_mode), 0o500)
        self.assertEqual(stat.S_IMODE(copied.parent.stat().st_mode), 0o500)
        original_bytes = copied.read_bytes()
        executable.write_bytes(b"replacement from another build")
        self.assertEqual(copied.read_bytes(), original_bytes)
        replacement = executable.with_suffix(".next")
        replacement.write_bytes(b"another replacement")
        os.replace(replacement, executable)
        self.assertEqual(copied.read_bytes(), original_bytes)
        prefix = "[taira-check] isolated native artifact "
        events = [json.loads(line[len(prefix):]) for line in self.stdout.getvalue().splitlines()
                  if line.startswith(prefix)]
        self.assertEqual(events, [{"selection": "iroha", "path": str(copied),
            "sha256": hashlib.sha256(original_bytes).hexdigest(), "size": len(original_bytes),
            "cargo_artifact": row}])

    def test_retirement_gets_only_verified_test_identities_under_cargo_lock(self):
        test, row, _ = self.artifact("core")
        production, production_row, _ = self.artifact("iroha")
        expected = gate.native_test_output_identity(test.stat())
        def retire(root, target, rows):
            self.assert_profile_locked()
            self.assertEqual((root, target), (self.source, self.target))
            self.assertEqual(set(rows), {"core"})
            self.assertEqual(rows["core"]["identity"], expected)
            self.assertEqual(rows["core"]["path"], str(test))
            self.assertTrue(production.exists())
        with patch.object(gate, "retire_superseded_native_test_outputs", side_effect=retire) as retirement:
            copies = self.isolate({"core": row, "iroha": production_row})
        self.addCleanup(copies.__exit__, None, None, None)
        retirement.assert_called_once()

    def test_corrupt_retirement_ledger_does_not_fail_a_valid_capture(self):
        _, row, _ = self.artifact("core")
        copies = self.isolate({"core": row})
        copies.__exit__(None, None, None)
        ledger = next(self.target.glob("taira-native-test-outputs-*/ledger.json"))
        ledger.write_bytes(b"not-json")
        copies = self.isolate({"core": row})
        self.addCleanup(copies.__exit__, None, None, None)
        self.assertTrue(Path(copies["core"]).exists())
        self.assertIn("retirement skipped", self.stdout.getvalue())

    def superseded_harness(self):
        old, row, _ = self.artifact("core")
        with self.isolate({"core": row}):
            pass
        new = old.with_name(gate.HARNESS_TARGETS["core"][1] + "-ffffffffffffffff")
        new.write_bytes(b"new verified executable")
        new.chmod(0o700)
        return old, new, row | {"executable": str(new)}

    def test_verified_retirement_frees_reserve_before_new_copy(self):
        old, new, row = self.superseded_harness()
        observed = []
        def capacity(path):
            self.assert_profile_locked()
            observed.append(old.exists())
            return MagicMock(free=gate.NETWORK_FIXTURE_FREE_BYTES + new.stat().st_size
                             - int(old.exists()))
        with patch.object(gate, "native_artifact_clone_function", return_value=None), \
             patch.object(gate.shutil, "disk_usage", side_effect=capacity):
            with self.isolate({"core": row}) as copies:
                self.assertEqual(Path(copies["core"]).read_bytes(), new.read_bytes())
        self.assertTrue(observed)
        self.assertFalse(any(observed), "reserve checks must observe already-freed predecessors")
        self.assertFalse(old.exists())
        self.assertTrue(new.exists())

    def test_busy_predecessor_retained_and_reserve_failure_is_accurate(self):
        old, new, row = self.superseded_harness()
        self.closed.return_value = False
        outputs = set(self.target.glob("taira-native-artifacts-*"))
        available = gate.NETWORK_FIXTURE_FREE_BYTES + new.stat().st_size - 1
        with patch.object(gate, "native_artifact_clone_function", return_value=None), \
             patch.object(gate.shutil, "disk_usage", return_value=MagicMock(free=available)):
            with self.assertRaisesRegex(gate.CheckError, "working-space reserve"):
                self.isolate({"core": row})
        self.assertTrue(old.exists())
        self.assertTrue(new.exists())
        self.assertEqual(set(self.target.glob("taira-native-artifacts-*")), outputs)
        self.assertIn("superseded test outputs: busy, changed, or unverified", self.stdout.getvalue())

    def test_copy_failure_after_verified_retirement_keeps_current_producer(self):
        old, new, row = self.superseded_harness()
        def failed_clone(*args):
            self.assert_profile_locked()
            self.assertFalse(old.exists(), "verified retirement must precede copy attempts")
            raise OSError(errno.EIO, "fixture copy failed")
        with patch.object(gate, "native_artifact_clone_function", return_value=failed_clone):
            with self.assertRaisesRegex(gate.CheckError, "fixture copy failed"):
                self.isolate({"core": row})
        self.assertFalse(old.exists())
        self.assertTrue(new.exists())
        ledger = next(self.target.glob("taira-native-test-outputs-*/ledger.json"))
        self.assertEqual(json.loads(ledger.read_text())["current"]["core"]["path"], str(new))
        self.assert_profile_unlocked()

    def test_later_invalid_identity_blocks_all_retirement(self):
        rows = {name: self.artifact(name)[1] for name in ("core", "cli")}
        original = self.contract.stable_hash_path
        def verify(path, **kwargs):
            if path == Path(rows["cli"]["executable"]):
                raise OSError("later artifact identity invalid")
            return original(path, **kwargs)
        with patch.object(self.contract, "stable_hash_path", side_effect=verify), \
             patch.object(gate, "retire_superseded_native_test_outputs") as retire:
            with self.assertRaisesRegex(gate.CheckError, "later artifact identity invalid"):
                self.isolate(rows)
        retire.assert_not_called()
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])

    @unittest.skipUnless(sys.platform == "darwin", "requires native macOS fclonefileat")
    def test_native_clone_preserves_bytes_after_source_write_and_replacement(self):
        payload = bytes(range(256)) * 4096
        executable, row, _ = self.artifact(payload=payload)
        clone = gate.native_artifact_clone_function()
        self.assertIsNotNone(clone)
        cloned = []
        def observe(*args):
            result = clone(*args)
            cloned.append(result)
            return result
        with patch.object(gate, "native_artifact_clone_function", return_value=observe):
            copies = self.isolate({"iroha": row})
        if cloned == [False]:
            self.skipTest("fixture filesystem does not support native clones")
        self.assertEqual(cloned, [True])
        destination = Path(copies["iroha"])
        self.assertNotEqual(executable.stat().st_ino, destination.stat().st_ino)
        self.assertEqual(destination.stat().st_nlink, 1)
        self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o500)
        self.assertEqual(destination.read_bytes(), payload)
        with executable.open("r+b") as source:
            source.write(b"MUTATED-SOURCE")
            source.flush()
            os.fsync(source.fileno())
        self.assertEqual(destination.read_bytes(), payload)
        replacement = executable.with_suffix(".next")
        replacement.write_bytes(b"REPLACED-SOURCE")
        os.replace(replacement, executable)
        self.assertEqual(destination.read_bytes(), payload)

    @unittest.skipUnless(sys.platform == "darwin", "requires native macOS fclonefileat")
    def test_native_clone_accepts_owner_read_execute_source(self):
        executable, row, _ = self.artifact()
        payload = executable.read_bytes()
        executable.chmod(0o500)
        clone = gate.native_artifact_clone_function()
        self.assertIsNotNone(clone)
        cloned = []
        def observe(*args):
            result = clone(*args)
            cloned.append(result)
            return result
        with patch.object(gate, "native_artifact_clone_function", return_value=observe):
            copies = self.isolate({"iroha": row})
        if cloned == [False]:
            self.skipTest("fixture filesystem does not support native clones")
        self.assertEqual(cloned, [True])
        destination = Path(copies["iroha"])
        self.assertEqual(destination.read_bytes(), payload)
        self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o500)
        self.assertNotEqual(executable.stat().st_ino, destination.stat().st_ino)

    def test_unsupported_clone_streams_when_full_capacity_is_available(self):
        executable, row, _ = self.artifact()
        with patch.object(gate, "native_artifact_clone_function", return_value=lambda *args: False):
            copies = self.isolate({"iroha": row})
        destination = Path(copies["iroha"])
        self.assertEqual(destination.read_bytes(), executable.read_bytes())
        self.assertNotEqual(destination.stat().st_ino, executable.stat().st_ino)

    def test_unsupported_clone_requires_capacity_for_all_remaining_full_copies(self):
        executable, row, _ = self.artifact(payload=b"x" * 1024 * 1024)
        other, other_row, _ = self.artifact("iroha3d", payload=b"y" * 1024 * 1024)
        adequate = MagicMock(free=gate.NETWORK_FIXTURE_FREE_BYTES + 64 * 1024 * 1024)
        inadequate = MagicMock(free=gate.NETWORK_FIXTURE_FREE_BYTES
                               + executable.stat().st_size + other.stat().st_size - 1)
        with patch.object(gate, "native_artifact_clone_function", return_value=lambda *args: False), \
             patch.object(gate.shutil, "disk_usage", side_effect=[adequate, adequate, inadequate]):
            with self.assertRaisesRegex(gate.CheckError, "working-space reserve"):
                self.isolate({"iroha": row, "iroha3d": other_row})
        self.assertFalse(any(self.target.glob("taira-native-artifacts-*/iroha")))
        self.assertFalse(any(self.target.glob("taira-native-artifacts-*/iroha3d")))
        self.assertNotIn("isolated native artifact", self.stdout.getvalue())
        self.assert_profile_unlocked()

    def test_clone_io_failure_never_streams_or_publishes(self):
        _, row, _ = self.artifact()
        def fail(*args):
            raise OSError(errno.EIO, "fixture clone I/O failure")
        with patch.object(gate, "native_artifact_clone_function", return_value=fail):
            with self.assertRaisesRegex(gate.CheckError, "fixture clone I/O failure"):
                self.isolate({"iroha": row})
        self.assertFalse(any(self.target.glob("taira-native-artifacts-*/iroha")))
        self.assertNotIn("isolated native artifact", self.stdout.getvalue())
        self.assert_profile_unlocked()

    def test_second_copy_failure_cleans_only_verified_closed_unpublished_copies(self):
        for state in ("closed", "busy", "unverified", "replaced"):
            with self.subTest(state=state):
                rows = {name: self.artifact(name, ("producer-" + name).encode())[1]
                        for name in ("iroha", "iroha3d")}
                before = set(self.target.glob("taira-native-artifacts-*"))
                self.closed.return_value = {"busy": False, "unverified": None}.get(state, True)
                def second_copy_fails(source, directory, name):
                    if name == "iroha":
                        return False  # Let the first copy pass the actual streamed verification.
                    output, = set(self.target.glob("taira-native-artifacts-*")) - before
                    first = output / "iroha"
                    self.assertEqual(first.read_bytes(), b"producer-iroha")
                    if state == "replaced":
                        first.unlink()
                        first.write_bytes(b"foreign replacement")
                        first.chmod(0o500)
                    (output / name).write_bytes(b"incomplete second copy")
                    (output / "unrecorded").write_bytes(b"unowned diagnostic")
                    raise OSError(errno.EIO, "second artifact copy failed")
                errors = io.StringIO()
                with patch.object(gate, "native_artifact_clone_function", return_value=second_copy_fails), \
                     contextlib.redirect_stderr(errors):
                    with self.assertRaisesRegex(gate.CheckError, "second artifact copy failed"):
                        self.isolate(rows)
                self.closed.return_value = True
                output, = set(self.target.glob("taira-native-artifacts-*")) - before
                if state == "closed":
                    self.assertFalse((output / "iroha").exists(), "unpublished CLI is not retained for operators")
                else:
                    expected = b"foreign replacement" if state == "replaced" else b"producer-iroha"
                    self.assertEqual((output / "iroha").read_bytes(), expected)
                    self.assertIn("retained", errors.getvalue())
                self.assertEqual((output / "iroha3d").read_bytes(), b"incomplete second copy")
                self.assertEqual((output / "unrecorded").read_bytes(), b"unowned diagnostic")
                for name, row in rows.items():
                    self.assertEqual(Path(row["executable"]).read_bytes(), ("producer-" + name).encode())
                self.assertNotIn("isolated native artifact", self.stdout.getvalue())
                self.assert_profile_unlocked()

    def test_publication_failure_closes_owner_and_discards_unpublished_copies(self):
        original_owner, original_print = gate.NativeArtifactCopies, print
        for failed_observation in (1, 2):
            with self.subTest(failed_observation=failed_observation):
                rows = {name: self.artifact(name, ("producer-" + name).encode())[1]
                        for name in ("iroha", "iroha3d")}
                before = set(self.target.glob("taira-native-artifacts-*"))
                owners, descriptors, observed = [], [], []
                failure = BrokenPipeError(errno.EPIPE, "fixture publication channel failed")
                def track_owner(*args, **kwargs):
                    owner = original_owner(*args, **kwargs)
                    owners.append(owner)
                    descriptors.append(owner.directory_fd)
                    return owner
                def fail_publication(*args, **kwargs):
                    if args and str(args[0]).startswith("[taira-check] isolated native artifact "):
                        self.assertTrue(owners, "publication begins only after constructing the owner")
                        self.assertIsNotNone(owners[0].directory_fd)
                        observed.append(args[0])
                        if len(observed) == failed_observation:
                            raise failure
                    return original_print(*args, **kwargs)
                with patch.object(gate, "NativeArtifactCopies", side_effect=track_owner), \
                     patch("builtins.print", side_effect=fail_publication):
                    with self.assertRaisesRegex(gate.CheckError, "fixture publication channel failed") as raised:
                        self.isolate(rows)
                self.assertIs(raised.exception.__cause__, failure)
                self.assertEqual(len(observed), failed_observation)
                self.assertTrue(all(owner.directory_fd is None for owner in owners))
                for descriptor in descriptors:
                    self.assertIsNotNone(descriptor)
                    with self.assertRaises(OSError):
                        os.fstat(descriptor)
                output, = set(self.target.glob("taira-native-artifacts-*")) - before
                self.assertFalse((output / "iroha").exists())
                self.assertFalse((output / "iroha3d").exists())
                for name, row in rows.items():
                    self.assertEqual(Path(row["executable"]).read_bytes(), ("producer-" + name).encode())
                self.assert_profile_unlocked()

    def test_strict_foreign_fingerprints_reject_without_retirement_or_publication(self):
        _, row, _ = self.artifact()
        directory = self.target / "debug" / ".fingerprint" / "iroha_cli-0123456789abcdef"
        directory.mkdir(parents=True)
        path = b"src/main.rs"
        raw = b"\x01\x00\x00\x00\xff\x01" + struct.pack("<I", 1)
        raw += b"\x00" + struct.pack("<I", len(path)) + path + b"\x00" + struct.pack("<I", 0)
        record = directory / "dep-bin-iroha"
        record.write_bytes(raw)
        record.chmod(0o600)
        with self.assertRaisesRegex(gate.CheckError, "foreign Cargo source fingerprints"):
            self.isolate({"iroha": row})
        self.assertEqual(record.read_bytes(), raw)
        self.assertFalse((self.target / "taira-release-cache-retired").exists())
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])
        self.assertNotIn("isolated native artifact", self.stdout.getvalue())
        self.assert_profile_unlocked()

    def test_unsafe_manifest_and_paths_fail_before_metadata_or_copy(self):
        executable, row, _ = self.artifact()
        outside = self.directory / "outside"
        outside.write_bytes(b"outside"); outside.chmod(0o700)
        symlink = self.target / "debug" / "link"
        symlink.symlink_to(executable)
        directory_link = self.target / "linked-debug"
        directory_link.symlink_to(self.target / "debug", target_is_directory=True)
        for changed in (row | {"manifest_path": str(self.directory / "foreign/Cargo.toml")},
                        row | {"executable": str(outside)}, row | {"executable": "relative"},
                        row | {"executable": str(symlink)},
                        row | {"executable": str(directory_link / "iroha")}):
            with self.subTest(record=changed), self.assertRaises(gate.CheckError):
                self.isolate({"iroha": changed})
        self.metadata.assert_not_called()
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])

    def test_hardlink_nonexecutable_unsafe_mode_empty_and_nonregular_are_rejected(self):
        executable, row, _ = self.artifact()
        for mode in (0o600, 0o722):
            executable.chmod(mode)
            with self.subTest(mode=mode), self.assertRaises(gate.CheckError):
                self.isolate({"iroha": row})
        executable.chmod(0o700)
        sibling = executable.with_suffix(".linked")
        os.link(executable, sibling)
        with self.assertRaises(gate.CheckError): self.isolate({"iroha": row})
        sibling.unlink()
        executable.write_bytes(b"")
        with self.assertRaises(gate.CheckError): self.isolate({"iroha": row})
        executable.unlink(); executable.mkdir()
        with self.assertRaises(gate.CheckError): self.isolate({"iroha": row})
        executable.rmdir(); os.mkfifo(executable)
        with self.assertRaises(gate.CheckError): self.isolate({"iroha": row})
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])

    def test_replacement_after_validation_is_rejected_before_execution(self):
        executable, row, _ = self.artifact()
        original = self.contract.stable_hash_path
        def replace_after_hash(*args, **kwargs):
            result = original(*args, **kwargs)
            replacement = executable.with_suffix(".next")
            replacement.write_bytes(b"foreign executable"); replacement.chmod(0o700)
            os.replace(replacement, executable)
            return result
        with patch.object(self.contract, "stable_hash_path", side_effect=replace_after_hash):
            with self.assertRaisesRegex(gate.CheckError, "stable capture"):
                self.isolate({"iroha": row})
        self.assertNotIn("isolated native artifact", self.stdout.getvalue())
        self.assert_profile_unlocked()

    def test_same_size_mutation_during_descriptor_copy_is_rejected(self):
        executable, row, _ = self.artifact()
        original_open, original_read = self.contract.stable_open_relative, os.read
        active = {"fd": None, "changed": False}
        @contextlib.contextmanager
        def track_open(*args, **kwargs):
            with original_open(*args, **kwargs) as fd:
                active["fd"] = fd
                try: yield fd
                finally: active["fd"] = None
        def mutate_after_read(fd, count):
            result = original_read(fd, count)
            if fd == active["fd"] and result and not active["changed"]:
                active["changed"] = True
                executable.write_bytes(b"x" * len(result))
            return result
        with patch.object(gate, "native_artifact_clone_function", return_value=None), \
             patch.object(self.contract, "stable_open_relative", side_effect=track_open), \
             patch.object(gate.os, "read", side_effect=mutate_after_read):
            with self.assertRaisesRegex(gate.CheckError, "changed while"):
                self.isolate({"iroha": row})
        self.assertTrue(active["changed"])
        self.assertNotIn("isolated native artifact", self.stdout.getvalue())
        self.assert_profile_unlocked()

    def test_destination_replacement_before_publication_does_not_emit_artifact(self):
        _, row, _ = self.artifact()
        original_open = self.contract.stable_open_relative
        @contextlib.contextmanager
        def replace_destination_after_copy(*args, **kwargs):
            with original_open(*args, **kwargs) as fd:
                yield fd
            output, = self.target.glob("taira-native-artifacts-*")
            destination = output / "iroha"
            replacement = output / "replacement"
            replacement.write_bytes(b"foreign"); replacement.chmod(0o500)
            os.replace(replacement, destination)
        with patch.object(self.contract, "stable_open_relative", side_effect=replace_destination_after_copy):
            with self.assertRaisesRegex(gate.CheckError, "before publication"):
                self.isolate({"iroha": row})
        self.assertNotIn("isolated native artifact", self.stdout.getvalue())
        self.assert_profile_unlocked()

    def test_capacity_reserve_failure_does_not_publish_or_create_copy_directory(self):
        executable, row, _ = self.artifact()
        space = MagicMock(free=gate.NETWORK_FIXTURE_FREE_BYTES + executable.stat().st_size - 1)
        with patch.object(gate, "native_artifact_clone_function", return_value=None), \
             patch.object(gate.shutil, "disk_usage", return_value=space):
            with self.assertRaisesRegex(gate.CheckError, "working-space reserve"):
                self.isolate({"iroha": row})
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])

    def process(self, events, code=0):
        child = MagicMock()
        child.stdout = io.StringIO("\n".join(json.dumps(event) for event in events))
        child.wait.return_value = code
        process = MagicMock()
        process.__enter__.return_value = child
        return process

    def test_every_harness_and_network_binary_uses_an_isolated_execution_path(self):
        for selection in gate.HARNESS_TARGETS:
            with self.subTest(selection=selection):
                executable, _, event = self.artifact(selection)
                with patch.object(gate.subprocess, "Popen", return_value=self.process([event])):
                    copies = gate.compile_harness(self.source, self.env, harness=selection)
                    self.addCleanup(copies.__exit__, None, None, None)
                    path = copies[selection]
                self.assertNotEqual(path, str(executable))
                self.assertTrue(Path(path).parent.name.startswith("taira-native-artifacts-"))
                self.assertEqual(Path(path).read_bytes(), executable.read_bytes())
        rows = [self.artifact(selection, shipping=True) for selection in ("iroha3d", "iroha", "taira-launcher")]
        with patch.object(gate.subprocess, "Popen", return_value=self.process([row[2] for row in rows])):
            copied = gate.compile_network_binaries(self.source, self.env, (77,))
        self.assertEqual(set(copied), {"iroha3d", "iroha", "taira-launcher"})
        for selection, path in copied.items():
            self.assertNotEqual(path, str(self.target / "debug" / selection))
        self.assert_profile_unlocked()

    def test_batched_libraries_and_integrations_copy_every_accepted_artifact_before_returning(self):
        selections = ("crypto", "p2p", "core", "test-network", "client", "wallet", "torii-unit", "torii", "torii-shared", "torii-lifecycle", "network")
        events = [self.artifact(selection)[2] for selection in selections]
        with patch.object(gate.subprocess, "Popen", return_value=self.process(events)) as cargo:
            copies = gate.compile_test_harnesses(self.source, self.env,
                                                   harnesses=selections, lock_fds=(77, 88))
        self.assertEqual(cargo.call_count, 1)
        self.addCleanup(copies.__exit__, None, None, None)
        self.assertEqual(set(copies), set(selections))
        self.assertEqual(len({Path(path).parent for path in copies.values()}), 1)
        for selection, path in copies.items():
            self.assertEqual(Path(path).name, selection)
            self.assertEqual(stat.S_IMODE(Path(path).stat().st_mode), 0o500)
        self.assert_profile_unlocked()

    def test_real_harness_exit_releases_copy_on_success_and_failure_with_logs_retained(self):
        for succeeds in (True, False):
            with self.subTest(succeeds=succeeds):
                payload = (f"#!{sys.executable}\nimport sys\nfrom pathlib import Path\n"
                    "assert Path(sys.argv[0]).is_file()\n"
                    "if '--list' in sys.argv: print('fixture: test'); sys.exit(0)\n"
                    + ("print('test fixture ... ok\\ntest result: ok. 1 passed; 0 failed; 0 ignored;')\n"
                       if succeeds else "print('diagnostic retained'); sys.exit(101)\n")).encode()
                original, row, event = self.artifact("core", payload)
                with patch.object(gate.subprocess, "Popen", return_value=self.process([event])):
                    copies = gate.compile_harness(self.source, self.env, harness="core")
                copied = Path(copies["core"])
                error_output = io.StringIO()
                expected = contextlib.nullcontext() if succeeds else self.assertRaisesRegex(gate.CheckError, "fixture.*101")
                with expected, contextlib.redirect_stderr(error_output):
                    with copies:
                        gate.run_stages(copies["core"], self.target, dict(os.environ),
                                        (("real child", ("fixture",)),), ())
                        self.assertTrue(copied.exists(), "copy lives until the child has completed")
                self.assertFalse(copied.exists())
                self.assertEqual(original.read_bytes(), payload)
                self.assertEqual(stat.S_IMODE(copied.parent.stat().st_mode), 0o500)
                self.assertIn('released native artifact', self.stdout.getvalue())
                self.assertIn(hashlib.sha256(payload).hexdigest(), self.stdout.getvalue())
                if not succeeds:
                    self.assertIn('diagnostic retained', error_output.getvalue())

    def test_batch_releases_temporary_copies_preserving_published_cli_and_producers(self):
        rows = {name: self.artifact(name)[1] for name in ("core", "cli", "network", "iroha", "iroha3d")}
        copies = self.isolate(rows)
        output = Path(copies["core"]).parent
        output.chmod(0o700)
        note = output / "diagnostic.txt"
        note.write_text("retain diagnostic")
        output.chmod(0o500)
        with self.assertRaisesRegex(gate.CheckError, "later fixture failed"):
            with copies:
                copies.release("core")
                self.assertFalse(Path(copies["core"]).exists())
                copies.release("cli")
                self.assertFalse(Path(copies["cli"]).exists(), "CLI test harness is temporary")
                copies.release("iroha")
                self.assertTrue(Path(copies["iroha"]).exists(), "published shipping CLI remains usable")
                self.assertTrue(Path(copies["network"]).exists())
                raise gate.CheckError("later fixture failed")
        self.assertFalse(Path(copies["network"]).exists())
        self.assertFalse(Path(copies["iroha3d"]).exists())
        self.assertTrue(Path(copies["iroha"]).exists())
        for row in rows.values():
            self.assertTrue(Path(row["executable"]).exists(), "Cargo output must remain warm")
        self.assertEqual(note.read_text(), "retain diagnostic")
        self.assertIsNone(copies.directory_fd)

    def test_network_copies_live_through_real_children_then_retain_only_published_cli(self):
        child_payload = (f"#!{sys.executable}\nimport os, sys\nfrom pathlib import Path\n"
                         "assert Path(sys.argv[0]).is_file()\n"
                         "for key in ('TEST_NETWORK_BIN_IROHAD', 'TEST_NETWORK_BIN_IROHAD_TAIRA', 'TEST_NETWORK_BIN_IROHA'):\n"
                         "    assert Path(os.environ[key]).is_file()\n"
                         "print('native child completed')\n").encode()
        for succeeds in (True, False):
            with self.subTest(succeeds=succeeds):
                rows = {name: self.artifact(name, child_payload, shipping=True)[1] for name in ("iroha", "iroha3d", "taira-launcher")}
                copies = self.isolate(rows)
                output = Path(copies["iroha"]).parent
                output.chmod(0o700)
                unrecorded = output / "unrecorded"
                unrecorded.write_bytes(b"leave unowned files")
                output.chmod(0o500)
                harness_payload = (f"#!{sys.executable}\nimport os, subprocess, sys\nfrom pathlib import Path\n"
                    "if '--list' in sys.argv: print('fixture: test'); sys.exit(0)\n"
                    "log = Path(os.environ['TEST_NETWORK_TMP_DIR']) / 'native.log'\n"
                    "with log.open('w') as stream:\n"
                    "    for key in ('TEST_NETWORK_BIN_IROHAD', 'TEST_NETWORK_BIN_IROHAD_TAIRA', 'TEST_NETWORK_BIN_IROHA'):\n"
                    "        subprocess.run([os.environ[key]], check=True, stdout=stream)\n"
                    + ("print('test fixture ... ok\\ntest result: ok. 1 passed; 0 failed; 0 ignored;')\n"
                       if succeeds else "print('network fixture failed after children'); sys.exit(101)\n")).encode()
                harness, _, _ = self.artifact("network", harness_payload)
                errors = io.StringIO()
                expected = contextlib.nullcontext() if succeeds else self.assertRaisesRegex(
                    gate.CheckError, "fixture.*101")
                with patch.object(gate, "compile_network_binaries", return_value=copies), \
                     contextlib.redirect_stderr(errors), expected:
                    gate.run_network_checks(self.source, self.target, dict(os.environ) | self.env, (),
                                            harness=str(harness), stages=(("real network child", ("fixture",)),))
                self.assertFalse(Path(copies["iroha3d"]).exists())
                self.assertTrue(Path(copies["iroha"]).exists())
                self.assertTrue(all(Path(row["executable"]).exists() for row in rows.values()))
                self.assertTrue(harness.exists())
                self.assertEqual(unrecorded.read_bytes(), b"leave unowned files")
                logs = list(self.target.glob("taira-consensus-check-*/native.log"))
                self.assertEqual(len(logs), 1 if succeeds else 2)
                self.assertTrue(all(log.read_text() == "native child completed\nnative child completed\nnative child completed\n"
                                    for log in logs))
                self.assertIsNone(copies.directory_fd)
                if not succeeds:
                    self.assertIn("network fixture failed after children", errors.getvalue())

    def test_busy_or_inconclusive_copies_are_retained_without_masking_fixture_failure(self):
        for closed in (False, None):
            for fails in (False, True):
                with self.subTest(closed=closed, fails=fails):
                    copies = self.isolate({name: self.artifact(name)[1] for name in ("iroha3d", "core")})
                    self.closed.side_effect = lambda path: closed if path.name == "iroha3d" else True
                    errors = io.StringIO()
                    expected = self.assertRaisesRegex(gate.CheckError, "original child failed") if fails else contextlib.nullcontext()
                    with expected, contextlib.redirect_stderr(errors):
                        with copies:
                            if fails:
                                raise gate.CheckError("original child failed")
                    self.closed.side_effect = None
                    self.assertTrue(Path(copies["iroha3d"]).exists())
                    self.assertFalse(Path(copies["core"]).exists())
                    self.assertIn("retained", errors.getvalue())
                    self.assertIsNone(copies.directory_fd)

    def test_network_capacity_or_fixture_failure_retains_only_published_cli(self):
        for failure in ("capacity", "fixture"):
            with self.subTest(failure=failure):
                rows = {name: self.artifact(name)[1] for name in ("iroha", "iroha3d")}
                copies = self.isolate(rows)
                owner = gate if failure == "capacity" else gate.tempfile
                function = "require_network_fixture_capacity" if failure == "capacity" else "mkdtemp"
                with patch.object(gate, "compile_network_binaries", return_value=copies), \
                     patch.object(gate, "run_stages") as run, \
                     patch.object(owner, function, side_effect=gate.CheckError(failure + " failed")):
                    with self.assertRaisesRegex(gate.CheckError, failure + " failed"):
                        gate.run_network_checks(self.source, self.target, self.env, (),
                                                harness="unused", stages=(("runtime", ("fixture",)),))
                run.assert_not_called()
                self.assertFalse(Path(copies["iroha3d"]).exists())
                self.assertTrue(Path(copies["iroha"]).exists())
                self.assertTrue(all(Path(row["executable"]).exists() for row in rows.values()))

    def test_copy_replaced_during_closed_check_is_retained(self):
        copies = self.isolate({name: self.artifact(name)[1] for name in ("iroha3d", "core")})
        copied = Path(copies["iroha3d"])
        def replace_during_check(path):
            if path == copied:
                copied.parent.chmod(0o700)
                copied.unlink()
                copied.write_bytes(b"foreign replacement during inspection")
                copied.chmod(0o500)
                copied.parent.chmod(0o500)
            return True
        with patch.object(gate, "native_test_output_confirmed_closed", side_effect=replace_during_check):
            with self.assertRaisesRegex(gate.CheckError, "changed before release: iroha3d"):
                with copies:
                    pass
        self.assertEqual(copied.read_bytes(), b"foreign replacement during inspection")
        self.assertFalse(Path(copies["core"]).exists())
        self.assertIsNone(copies.directory_fd)

    def test_failed_early_run_preserves_original_failure_and_releases_only_owned_batch(self):
        for replaced in (False, True):
            with self.subTest(replaced=replaced):
                rows = {name: self.artifact(name)[1] for name in ("cli", "client")}
                copies = self.isolate(rows)
                copied = Path(copies["cli"])
                def failed_stage(*args, **_kwargs):
                    self.assertEqual(args[0], copies["cli"])
                    if replaced:
                        copied.parent.chmod(0o700)
                        copied.unlink()
                        copied.write_bytes(b"foreign replacement")
                        copied.chmod(0o500)
                        copied.parent.chmod(0o500)
                    raise gate.CheckError("early CLI fixture failed")
                errors = io.StringIO()
                with contextlib.ExitStack() as stack:
                    for name in ("MV_OWNERSHIP_STAGES", "MV_EBR_STAGES", "MV_MAP_STAGES", "MV_ADMITTED_MAP_STAGES", "CONCREAD_STAGES", "CRYPTO_STAGES", "P2P_STAGES", "CORE_STAGES", "DAEMON_STAGES", "TEST_NETWORK_STAGES", "TORII_UNIT_STAGES", "TORII_STAGES", "TORII_SHARED_STAGES", "TORII_LIFECYCLE_STAGES", "NETWORK_STAGES"):
                        stack.enter_context(patch.object(gate, name, ()))
                    for name in ("run_pure_fsm_checks", "run_lifecycle_source_checks", "run_config_checks", "check_test_harnesses"):
                        stack.enter_context(patch.object(gate, name))
                    stack.enter_context(patch.object(gate, "compile_test_harnesses", return_value=copies))
                    network = stack.enter_context(patch.object(gate, "compile_network_binaries"))
                    stack.enter_context(patch.object(gate, "run_stages", side_effect=failed_stage))
                    stack.enter_context(contextlib.redirect_stderr(errors))
                    with self.assertRaisesRegex(gate.CheckError, "early CLI fixture failed"):
                        gate.run_checks(self.source, qualification_scope="full", environment=self.env | {"CARGO_HOME": "/isolated"},
                                        source_commit="a" * 40)
                if replaced:
                    self.assertEqual(copied.read_bytes(), b"foreign replacement")
                    self.assertIn("changed before release: cli", errors.getvalue())
                else:
                    self.assertFalse(copied.exists())
                self.assertFalse(Path(copies["client"]).exists())
                self.assertTrue(all(Path(row["executable"]).exists() for row in rows.values()))
                network.assert_not_called()
                self.assertIsNone(copies.directory_fd)

    def test_replaced_test_copy_is_retained_and_other_owned_copy_is_released(self):
        copies = self.isolate({name: self.artifact(name)[1] for name in ("core", "network")})
        original = Path(copies["core"])
        output = original.parent
        output.chmod(0o700)
        original.unlink()
        original.write_bytes(b"foreign replacement")
        original.chmod(0o500)
        output.chmod(0o500)
        with self.assertRaisesRegex(gate.CheckError, "changed before release: core"):
            with copies:
                pass
        self.assertEqual(original.read_bytes(), b"foreign replacement")
        self.assertFalse(Path(copies["network"]).exists())
        self.assertIsNone(copies.directory_fd)

    def test_changed_directory_never_deletes_replacement_or_masks_test_failure(self):
        copies = self.isolate({"core": self.artifact("core")[1]})
        output = Path(copies["core"]).parent
        archived = output.with_name(output.name + "-renamed")
        output.rename(archived)
        output.mkdir(mode=0o700)
        foreign = output / "core"
        foreign.write_bytes(b"foreign")
        foreign.chmod(0o500)
        output.chmod(0o500)
        errors = io.StringIO()
        with self.assertRaisesRegex(gate.CheckError, "actual regression failed"), contextlib.redirect_stderr(errors):
            with copies:
                raise gate.CheckError("actual regression failed")
        self.assertEqual(foreign.read_bytes(), b"foreign")
        self.assertTrue((archived / "core").exists())
        self.assertIn("directory changed before release", errors.getvalue())
        self.assertIsNone(copies.directory_fd)

    def test_empty_and_unknown_selections_reject_before_creating_output(self):
        for rows in ({}, {"../foreign": {}}):
            with self.assertRaisesRegex(gate.CheckError, "known nonempty"):
                self.isolate(rows)
        self.metadata.assert_not_called()
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])

    def test_cargo_failure_and_ambiguous_metadata_never_copy(self):
        _, _, event = self.artifact("core")
        for events, code in (([event], 101),
                             ([event, event | {"manifest_path": "/other/Cargo.toml"}], 0)):
            with patch.object(gate.subprocess, "Popen", return_value=self.process(events, code)), \
                 patch.object(gate, "isolate_native_artifacts") as isolate:
                with self.assertRaises(gate.CheckError):
                    gate.compile_harness(self.source, self.env, harness="core")
                isolate.assert_not_called()

    def test_mutable_development_copy_uses_real_profile_lock_without_source_claim(self):
        executable, row, _ = self.artifact()
        development = self.directory / "checkout"
        development.mkdir(mode=0o700)
        row["manifest_path"] = str(development / "crates/iroha_cli/Cargo.toml")
        original = self.contract.stable_hash_path
        def guarded_hash(*args, **kwargs):
            self.assert_profile_locked()
            return original(*args, **kwargs)
        with patch.object(self.contract, "stable_hash_path", side_effect=guarded_hash):
            copied = gate.isolate_native_artifacts(development, self.env, {"iroha": row})
        self.assertNotEqual(copied["iroha"], str(executable))
        self.metadata.assert_not_called()
        self.assert_profile_unlocked()

class PureFsmGateTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.target = Path(self.directory.name).resolve()
        self.env = {"RUSTC": "/pinned/rustc", "CARGO_TARGET_DIR": str(self.target)}

    def test_standalone_compile_list_and_execution_use_private_child_umask(self):
        real_run = subprocess.run
        outputs = ("", "one: test\n", "test one ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n")
        calls = []

        def child(command, **kwargs):
            index = len(calls)
            calls.append(command)
            directory = self.target / f"child-{index}"
            source = (f"import os; os.mkdir({str(directory)!r},0o777); "
                      f"os.close(os.open({str(directory / 'owned-lock')!r},os.O_CREAT|os.O_WRONLY,0o666)); "
                      f"print({outputs[index]!r},end='')")
            return real_run([sys.executable, "-c", source], **kwargs)

        original_umask = os.umask(0o002)
        try:
            with patch.object(gate.subprocess, "run", side_effect=child), contextlib.redirect_stdout(io.StringIO()):
                gate.run_pure_fsm_checks(Path("/frozen"), self.env, ())
            self.assertEqual(os.umask(0o002), 0o002)
        finally:
            os.umask(original_umask)
        self.assertEqual(len(calls), 3)
        for index in range(3):
            self.assertEqual(stat.S_IMODE((self.target / f"child-{index}").stat().st_mode), 0o700)
            self.assertEqual(stat.S_IMODE((self.target / f"child-{index}/owned-lock").stat().st_mode), 0o600)

    @staticmethod
    def results(output=None, code=0):
        return [subprocess.CompletedProcess([], 0, "", ""),
                subprocess.CompletedProcess([], 0, "one: test\ntwo: test\n", ""),
                subprocess.CompletedProcess([], code, output if output is not None else
                    "test two ... ok\ntest one ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n", "")]

    def test_exact_production_source_all_tests_and_lock_custody(self):
        output = io.StringIO()
        with patch.object(gate.subprocess, "run", side_effect=self.results()) as run, contextlib.redirect_stdout(output):
            gate.run_pure_fsm_checks(Path("/frozen"), self.env, (77, 88))
        executable = str(self.target / "taira-consensus-fsm-check/sumeragi-core-tests")
        self.assertEqual(run.call_args_list[0].args[0], ["/pinned/rustc", "--edition=2024", "--test",
            "/frozen/crates/iroha_sumeragi_core/src/lib.rs", "-o", executable])
        self.assertEqual(run.call_args_list[1].args[0], [executable, "--list", "--format", "terse"])
        self.assertEqual(run.call_args_list[2].args[0], [executable, "--color", "never", "--test-threads=6"])
        for call in run.call_args_list:
            self.assertEqual(call.kwargs["pass_fds"], (77, 88))
            self.assertEqual(call.kwargs["env"], self.env)
            self.assertEqual(call.kwargs["cwd"], "/")
        self.assertIn("pure FSM PASS: 2 listed, 2 passed, 0 ignored", output.getvalue())
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_standalone_rustc_uses_exact_coordinated_native_linker_pair(self):
        for flags in (
            {"RUSTFLAGS": "-Clinker=/fixed/clang -Clink-arg=-fuse-ld=/fixed/ld.lld"},
            {"CARGO_ENCODED_RUSTFLAGS": "-Clinker=/Xcode Beta.app/clang\x1f-Clink-arg=-fuse-ld=/Xcode Beta.app/ld"},
        ):
            expected = (flags["RUSTFLAGS"].split() if "RUSTFLAGS" in flags else
                        flags["CARGO_ENCODED_RUSTFLAGS"].split("\x1f"))
            with self.subTest(flags=flags), patch.object(gate.subprocess, "run", side_effect=self.results()) as run, \
                 contextlib.redirect_stdout(io.StringIO()):
                gate.run_pure_fsm_checks(Path("/frozen"), self.env | flags, (77, 88))
            self.assertEqual(run.call_args_list[0].args[0][:5], ["/pinned/rustc", *expected, "--edition=2024", "--test"])
            self.assertEqual(run.call_args_list[0].kwargs["env"], self.env | flags)
            self.assertEqual(run.call_args_list[0].kwargs["pass_fds"], (77, 88))
            self.assertNotIn(expected[0], run.call_args_list[1].args[0])
            self.assertNotIn(expected[0], run.call_args_list[2].args[0])

    def test_standalone_rustc_rejects_extra_ambiguous_or_relative_flags_before_compilation(self):
        pair = "-Clinker=/fixed/clang\x1f-Clink-arg=-fuse-ld=/fixed/ld"
        for flags in (
            {"CARGO_ENCODED_RUSTFLAGS": pair, "RUSTFLAGS": ""},
            {"CARGO_ENCODED_RUSTFLAGS": pair + "\x1f--cfg=unreviewed"},
            {"RUSTFLAGS": "-Clinker=/fixed/clang"},
            {"CARGO_ENCODED_RUSTFLAGS": pair.replace("/fixed/clang", "relative-clang")},
            {"CARGO_ENCODED_RUSTFLAGS": pair.replace("/fixed/ld", "/fixed/../ld")},
            {"CARGO_ENCODED_RUSTFLAGS": "--cfg=unreviewed\x1f-Clink-arg=-fuse-ld=/fixed/ld"},
            {"CARGO_ENCODED_RUSTFLAGS": pair.replace("/fixed/ld", "/fixed/ld\n")},
        ):
            with self.subTest(flags=flags), patch.object(gate.subprocess, "run") as run, \
                 self.assertRaises(gate.CheckError):
                gate.run_pure_fsm_checks(Path("/frozen"), self.env | flags, ())
            run.assert_not_called()

    def test_empty_duplicate_or_malformed_census_never_executes_suite(self):
        for listing in ("", "one: test\none: test\n", "one: test\nother: benchmark\n"):
            results = self.results(); results[1] = subprocess.CompletedProcess([], 0, listing, "")
            with patch.object(gate.subprocess, "run", side_effect=results) as run, contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "census"):
                    gate.run_pure_fsm_checks(Path("/frozen"), self.env, ())
            self.assertEqual(run.call_count, 2)

    def test_lifecycle_source_gate_uses_captured_shared_assertions_and_same_locks(self):
        results = [subprocess.CompletedProcess([], 0, "", "source asset audit passed"),
                   subprocess.CompletedProcess([], 0, "", "native instruction audit passed")] + self.results()
        with patch.object(gate.subprocess, "run", side_effect=results) as run, \
             patch.object(gate, "validate_torii_lifecycle_test_registration") as registration, \
             contextlib.redirect_stdout(io.StringIO()) as output:
            gate.run_lifecycle_source_checks(Path("/frozen"), self.env, (77, 88))
        registration.assert_called_once_with(Path("/frozen"))
        self.assertEqual(run.call_args_list[0].args[0], [sys.executable, "-I", "-B",
            "/frozen/scripts/tests/sumeragi_source_contract_asset_compaction_test.py"])
        executable = str(self.target / "taira-consensus-fsm-check/lifecycle-source-tests")
        self.assertEqual(run.call_args_list[1].args[0], [sys.executable, "-I", "-B",
            "/frozen/scripts/check_taira_initial_executor.py", "--repo", "/frozen", "--self-test"])
        self.assertEqual(run.call_args_list[2].args[0], ["/pinned/rustc", "--edition=2024", "--test",
            "/frozen/crates/iroha_core/src/sumeragi/v2_lifecycle_source_contract_harness.rs",
            "-o", executable])
        for call in run.call_args_list:
            self.assertEqual(call.kwargs["pass_fds"], (77, 88))
            self.assertEqual(call.kwargs["env"], self.env)
            self.assertEqual(call.kwargs["cwd"], "/")
        self.assertIn("lifecycle source contracts PASS: 2 listed, 2 passed, 0 ignored", output.getvalue())
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_native_instruction_audit_failure_stops_before_rust_or_cargo(self):
        results = [subprocess.CompletedProcess([], 0, "", ""),
                   subprocess.CompletedProcess([], 1, "", "missing reviewed disposition\n")]
        with patch.object(gate.subprocess, "run", side_effect=results) as run, \
             patch.object(gate, "validate_torii_lifecycle_test_registration"), \
             patch.object(gate, "_run_standalone_checks") as rust, \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()) as error:
            with self.assertRaisesRegex(gate.CheckError, "native Initial instruction source audit failed"):
                gate.run_lifecycle_source_checks(Path("/frozen"), self.env, (77,))
        self.assertEqual(run.call_count, 2)
        rust.assert_not_called()
        self.assertEqual(error.getvalue(), "missing reviewed disposition\n")

    def test_asset_audit_failure_stops_before_rust_or_cargo_and_preserves_diagnostic(self):
        env = self.env | {"CARGO": "/pinned/cargo", "CARGO_HOME": "/isolated"}
        diagnostic = "AssertionError: invalid region edge: from/before\n"
        with patch.object(gate, "run_pure_fsm_checks"), \
             patch.object(gate, "validate_torii_lifecycle_test_registration"), \
             patch.object(gate.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", diagnostic)) as run, \
             patch.object(gate, "_run_standalone_checks") as rust, \
             patch.object(gate, "run_config_checks") as config, \
             patch.object(gate, "compile_test_harnesses") as libraries, \
             patch.object(gate, "run_network_checks") as network, \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()) as error:
            with self.assertRaisesRegex(gate.CheckError, "source-asset grammar and inventory audit failed"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args.kwargs["pass_fds"], (77,))
        self.assertEqual(run.call_args.kwargs["timeout"], 120)
        self.assertEqual(error.getvalue(), diagnostic)
        rust.assert_not_called()
        config.assert_not_called()
        libraries.assert_not_called()
        network.assert_not_called()
        self.assertEqual(gate.selected_regression_count("full"), EXPECTED_REGRESSION_COUNT)

    def test_lifecycle_failure_stops_before_any_cargo_or_network_work(self):
        env = self.env | {"CARGO": "/pinned/cargo", "CARGO_HOME": "/isolated"}
        with patch.object(gate, "run_pure_fsm_checks") as fsm, \
             patch.object(gate, "run_lifecycle_source_checks", side_effect=gate.CheckError("source contract failed")) as source, \
             patch.object(gate, "compile_test_harnesses") as libraries, \
             patch.object(gate, "compile_harness") as compile, \
             patch.object(gate, "run_network_checks") as network, \
             contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "source contract failed"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        fsm.assert_called_once()
        source.assert_called_once_with(Path("/frozen"),
            env | {"VERGEN_GIT_SHA": "a" * 40, "IROHA_GIT_COMMIT_HASH": "a" * 40}, (77,))
        libraries.assert_not_called()
        compile.assert_not_called()
        network.assert_not_called()

    def test_partial_ignored_substituted_duplicate_and_failed_results_rejected(self):
        good = self.results()[-1].stdout
        cases = [(good.replace("test two ... ok\n", ""), 0),
                 (good.replace("test two ... ok", "test other ... ok"), 0),
                 (good + "test one ... ok\n", 0),
                 (good.replace("2 passed; 0 failed; 0 ignored", "1 passed; 0 failed; 1 ignored"), 0),
                 (good, 101)]
        for text, code in cases:
            with patch.object(gate.subprocess, "run", side_effect=self.results(text, code)), \
                 contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "without skips"):
                    gate.run_pure_fsm_checks(Path("/frozen"), self.env, ())

    def test_compiler_failure_never_runs_stale_output(self):
        with patch.object(gate.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", "compile failure")) as run, \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "compilation failed"):
                gate.run_pure_fsm_checks(Path("/frozen"), self.env, ())
        self.assertEqual(run.call_count, 1)

    def test_fsm_failure_precedes_any_native_network_or_cargo_build(self):
        env = self.env | {"CARGO": "/pinned/cargo", "CARGO_HOME": "/isolated"}
        with patch.object(gate, "run_pure_fsm_checks", side_effect=gate.CheckError("FSM failed")) as fsm, \
             patch.object(gate, "run_network_checks") as network, patch.object(gate, "compile_harness") as compile, \
             contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "FSM failed"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        fsm.assert_called_once()
        self.assertEqual(fsm.call_args.args[2], (77,))
        network.assert_not_called(); compile.assert_not_called()

    def test_unpinned_compiler_and_symlink_output_rejected_before_compilation(self):
        with patch.object(gate.subprocess, "run") as run:
            for compiler in ("rustc", ""):
                with self.assertRaisesRegex(gate.CheckError, "pinned RUSTC"):
                    gate.run_pure_fsm_checks(Path("/frozen"), self.env | {"RUSTC": compiler}, ())
            (self.target / "taira-consensus-fsm-check").symlink_to(self.target, target_is_directory=True)
            with self.assertRaisesRegex(gate.CheckError, "direct directory"):
                gate.run_pure_fsm_checks(Path("/frozen"), self.env, ())
        run.assert_not_called()


class NativeTestOutputRetirementTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.target = Path(self.temp.name).resolve() / "warm"
        self.source = self.target / "source"
        self.source.mkdir(parents=True, mode=0o700)
        (self.target / "debug/deps").mkdir(parents=True, mode=0o700)
        self.closed = patch.object(gate, "native_test_output_confirmed_closed", return_value=True).start()
        self.addCleanup(patch.stopall)
        self.log = io.StringIO()
        redirect = contextlib.redirect_stdout(self.log)
        redirect.__enter__()
        self.addCleanup(redirect.__exit__, None, None, None)

    def artifact(self, generation, selection="core"):
        name = gate.HARNESS_TARGETS[selection][1]
        path = self.target / "debug/deps" / f"{name}-{generation:016x}"
        path.write_bytes(b"final test executable")
        path.chmod(0o700)
        return {"selection": selection, "path": str(path),
                "identity": gate.native_test_output_identity(path.stat()), "quarantine": None}

    def run_capture(self, *rows):
        gate.retire_superseded_native_test_outputs(self.source, self.target, {row["selection"]: row for row in rows})

    def ledger(self):
        path = next(self.target.glob("taira-native-test-outputs-*/ledger.json"))
        self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
        return path, json.loads(path.read_text())

    def test_first_capture_seeds_and_next_retires_only_known_predecessor(self):
        old, new, unrecorded = self.artifact(1), self.artifact(2), self.artifact(3)
        library = self.target / "debug/deps/libiroha_core.rlib"
        library.write_bytes(b"warm library")
        self.run_capture(old)
        self.assertTrue(Path(old["path"]).exists())
        self.closed.assert_not_called()
        self.run_capture(new)
        self.assertFalse(Path(old["path"]).exists())
        self.assertTrue(Path(new["path"]).exists())
        self.assertTrue(Path(unrecorded["path"]).exists())
        self.assertTrue(library.exists())
        self.assertEqual(self.ledger()[1]["pending"], [])
        self.run_capture(new)
        self.assertTrue(Path(new["path"]).exists())

    def test_partial_capture_keeps_unselected_current_and_same_path_replacement(self):
        old, other = self.artifact(1), self.artifact(1, "cli")
        self.run_capture(old, other)
        Path(old["path"]).unlink()
        replacement = self.artifact(1)
        self.run_capture(replacement)
        self.assertTrue(Path(replacement["path"]).exists())
        self.assertTrue(Path(other["path"]).exists())
        self.closed.assert_not_called()

    def test_busy_file_is_never_moved_and_is_retried_after_close(self):
        old, new = self.artifact(1), self.artifact(2)
        self.run_capture(old)
        self.closed.return_value = False
        with patch.object(gate.os, "rename", wraps=os.rename) as rename:
            self.run_capture(new)
            rename.assert_not_called()
        self.assertTrue(Path(old["path"]).exists())
        self.assertEqual(len(self.ledger()[1]["pending"]), 1)
        self.closed.return_value = True
        self.run_capture(new)
        self.assertFalse(Path(old["path"]).exists())

    def test_reader_racing_before_quarantine_is_retained_then_retried(self):
        old, new = self.artifact(1), self.artifact(2)
        self.run_capture(old)
        self.closed.side_effect = [True, False]
        self.run_capture(new)
        ledger, state = self.ledger()
        retained = ledger.parent / state["pending"][0]["quarantine"]
        self.assertTrue(retained.exists())
        self.assertEqual(retained.stat().st_ino, old["identity"][1])
        self.closed.side_effect = None
        self.run_capture(new)
        self.assertFalse(retained.exists())

    def test_inode_drift_symlink_and_hardlink_are_retained(self):
        for mutation, selection in (("replace", "core"), ("symlink", "cli"), ("hardlink", "torii-unit")):
            with self.subTest(mutation=mutation):
                old, new = self.artifact(11, selection), self.artifact(12, selection)
                self.run_capture(old)
                path = Path(old["path"])
                if mutation == "hardlink":
                    os.link(path, path.with_name(path.name + "-linked"))
                else:
                    path.unlink()
                    if mutation == "symlink":
                        path.symlink_to(new["path"])
                    else:
                        path.write_bytes(b"unrelated replacement")
                        path.chmod(0o700)
                self.run_capture(new)
                self.assertTrue(path.exists())
                self.assertTrue(Path(new["path"]).exists())

    def test_no_ownership_on_bad_current_output_or_foreign_ledger(self):
        row = self.artifact(1)
        invalid = dict(row, path=str(self.target / "debug/iroha"))
        self.run_capture(invalid)
        self.assertEqual(list(self.target.glob("taira-native-test-outputs-*")), [])
        self.run_capture(row)
        path, state = self.ledger()
        state["source"] = "/different/source"
        path.write_text(json.dumps(state))
        self.run_capture(self.artifact(2))
        self.assertTrue(Path(row["path"]).exists())

    def test_ledger_failure_never_deletes_a_predecessor(self):
        old, new = self.artifact(1), self.artifact(2)
        self.run_capture(old)
        with patch.object(gate, "_save_native_test_outputs", side_effect=OSError("disk full")):
            self.run_capture(new)
        self.assertTrue(Path(old["path"]).exists())
        self.assertTrue(Path(new["path"]).exists())

    def test_interrupted_rename_refresh_resumes_only_after_reader_closes(self):
        old, new = self.artifact(1), self.artifact(2)
        self.run_capture(old)
        original_save = gate._save_native_test_outputs
        calls = 0
        def fail_after_rename(fd, state):
            nonlocal calls
            calls += 1
            if calls == 3:
                raise OSError("interrupted after rename")
            original_save(fd, state)
        with patch.object(gate, "_save_native_test_outputs", side_effect=fail_after_rename):
            self.run_capture(new)
        ledger, state = self.ledger()
        retained = ledger.parent / state["pending"][0]["quarantine"]
        self.assertTrue(retained.exists())
        self.assertEqual(retained.stat().st_ino, old["identity"][1])
        self.closed.return_value = False
        self.run_capture(new)
        self.assertTrue(retained.exists())
        self.closed.return_value = True
        self.run_capture(new)
        self.assertFalse(retained.exists())
        self.assertEqual(self.ledger()[1]["pending"], [])

    def test_interrupted_rename_intent_resumes_the_exact_original_file(self):
        old, new = self.artifact(1), self.artifact(2)
        self.run_capture(old)
        with patch.object(gate.os, "rename", side_effect=OSError("interrupted before rename")):
            self.run_capture(new)
        ledger, state = self.ledger()
        self.assertIsNotNone(state["pending"][0]["quarantine"])
        self.assertFalse((ledger.parent / state["pending"][0]["quarantine"]).exists())
        self.assertTrue(Path(old["path"]).exists())
        self.run_capture(new)
        self.assertFalse(Path(old["path"]).exists())
        self.assertEqual(self.ledger()[1]["pending"], [])

    def test_interrupted_rename_intent_never_adopts_replacement(self):
        old, new = self.artifact(1), self.artifact(2)
        self.run_capture(old)
        with patch.object(gate.os, "rename", side_effect=OSError("interrupted before rename")):
            self.run_capture(new)
        original = Path(old["path"])
        replacement = original.with_suffix(".replacement")
        replacement.write_bytes(original.read_bytes())
        replacement.chmod(0o700)
        os.replace(replacement, original)
        self.run_capture(new)
        self.assertTrue(original.exists())
        self.assertTrue(Path(new["path"]).exists())

    def test_interrupted_quarantine_refresh_never_adopts_replacement(self):
        old, new = self.artifact(1), self.artifact(2)
        self.run_capture(old)
        original_save = gate._save_native_test_outputs
        def fail_refresh(fd, state):
            pending = state["pending"]
            if pending and pending[0]["quarantine"] and not Path(old["path"]).exists():
                raise OSError("interrupted before refreshed identity publication")
            original_save(fd, state)
        with patch.object(gate, "_save_native_test_outputs", side_effect=fail_refresh):
            self.run_capture(new)
        ledger, state = self.ledger()
        retained = ledger.parent / state["pending"][0]["quarantine"]
        replacement = retained.with_suffix(".replacement")
        replacement.write_bytes(retained.read_bytes())
        replacement.chmod(0o700)
        os.replace(replacement, retained)
        self.run_capture(new)
        self.assertTrue(retained.exists())
        self.assertTrue(Path(new["path"]).exists())

    def test_bounded_ledger_retains_when_full(self):
        old, new = self.artifact(1), self.artifact(2)
        self.run_capture(old)
        with patch.object(gate, "NATIVE_TEST_OUTPUT_MAX_RECORDS", 1):
            self.run_capture(new)
        self.assertTrue(Path(old["path"]).exists())
        self.assertEqual(self.ledger()[1]["current"]["core"], old)

    def test_full_ledger_recovers_when_previously_busy_predecessor_closes(self):
        first, second, third = self.artifact(1), self.artifact(2), self.artifact(3)
        with patch.object(gate, "NATIVE_TEST_OUTPUT_MAX_RECORDS", 2):
            self.run_capture(first)
            self.closed.return_value = False
            self.run_capture(second)
            self.assertEqual(len(self.ledger()[1]["pending"]), 1)
            self.run_capture(third)
            self.assertTrue(Path(first["path"]).exists())
            self.assertTrue(Path(second["path"]).exists())
            self.closed.return_value = True
            self.run_capture(third)
        self.assertFalse(Path(first["path"]).exists())
        self.assertFalse(Path(second["path"]).exists())
        self.assertTrue(Path(third["path"]).exists())
        self.assertEqual(self.ledger()[1]["pending"], [])
        self.assertEqual(self.ledger()[1]["current"]["core"], third)

    def test_inconclusive_os_query_stops_without_repeated_waits(self):
        old, other = self.artifact(1), self.artifact(2, "cli")
        self.run_capture(old, other)
        self.closed.return_value = None
        self.run_capture(self.artifact(3), self.artifact(4, "cli"))
        self.closed.assert_called_once()
        self.assertTrue(Path(old["path"]).exists())
        self.assertTrue(Path(other["path"]).exists())


class BeaconFixturePrerequisiteTests(unittest.TestCase):
    env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}

    def invoke(self, mode, scope, *, test=None, source_commit=None):
        if mode == "focused":
            gate.run_prequalification(
                Path("/unread"), qualification_scope=scope,
                focused_regressions=(test or "network=" + gate.BEACON_NETWORK_TEST,),
                environment=self.env, lock_fds=())
        else:
            gate.run_checks(Path("/unread"), qualification_scope=scope,
                            environment=self.env, source_commit=source_commit)

    def test_selected_test_names_control_root_and_capacity_prerequisites(self):
        observation = gate.NETWORK_OBSERVATION_STAGES[0][1][0]
        selections = (
            ((), False, False),
            ((("renamed observation subset", (observation,)),), False, False),
            ((("other runtime", ("fixture_runtime_case",)),), False, True),
            ((("mixed selection", (observation, gate.BEACON_NETWORK_TEST)),), True, True),
        )
        for stages, needs_root, needs_capacity in selections:
            with self.subTest(stages=stages), \
                 patch.object(gate, "beacon_fixture_root") as root, \
                 patch.object(gate, "require_network_fixture_capacity") as capacity:
                gate.require_network_fixture_prerequisites(Path("/warm"), stages)
            self.assertEqual(root.call_count, int(needs_root))
            self.assertEqual(capacity.call_count, int(needs_capacity))
            if needs_capacity:
                capacity.assert_called_once_with(Path("/warm"))

    def test_beacon_root_admission_precedes_source_and_compilation_in_each_gate(self):
        for mode in ("qualification", "focused"):
            for scope in gate.QUALIFICATION_SCOPES:
                for source_commit in ((None, "a" * 40) if mode == "qualification" else (None,)):
                    order = []
                    with self.subTest(mode=mode, scope=scope, source_commit=source_commit), \
                         patch.object(gate, "require_native_artifact_inspector"), \
                         patch.object(gate, "beacon_fixture_root", side_effect=lambda: order.append("root")), \
                         patch.object(gate, "require_network_fixture_capacity", side_effect=lambda path: order.append(("capacity", path))), \
                         patch.object(gate.subprocess, "check_output", side_effect=lambda *a, **k: order.append("head") or "a" * 40 + "\n"), \
                         patch.object(gate, "run_pure_fsm_checks", side_effect=RuntimeError("first compile reached")) as compile, \
                         contextlib.redirect_stdout(io.StringIO()):
                        with self.assertRaisesRegex(RuntimeError, "first compile reached"):
                            self.invoke(mode, scope, source_commit=source_commit)
                        compile.assert_called_once()
                    directory = Path("/warm") if mode == "focused" or source_commit else Path("/unread")
                    self.assertEqual(order, ["root", ("capacity", directory)] + ([] if source_commit else ["head"]))

    def test_unselected_beacon_and_observation_only_focus_skip_peer_prerequisites(self):
        selections = ("cli=" + gate.STAGES[0][1][0],
                      "network=" + gate.NETWORK_OBSERVATION_STAGES[0][1][0])
        for scope in gate.QUALIFICATION_SCOPES:
            for selected in selections:
                with self.subTest(scope=scope, selected=selected), \
                     patch.object(gate, "require_native_artifact_inspector"), \
                     patch.object(gate, "beacon_fixture_root") as root, \
                     patch.object(gate, "require_network_fixture_capacity") as capacity, \
                     patch.object(gate.subprocess, "check_output", return_value="a" * 40 + "\n"), \
                     patch.object(gate, "run_pure_fsm_checks", side_effect=RuntimeError("first compile reached")), \
                     contextlib.redirect_stdout(io.StringIO()):
                    with self.assertRaisesRegex(RuntimeError, "first compile reached"):
                        self.invoke("focused", scope, test=selected)
                    root.assert_not_called()
                    capacity.assert_not_called()

    def test_unsafe_parent_stops_selected_gates_before_source_or_compile(self):
        with tempfile.TemporaryDirectory(dir=Path.home().resolve()) as directory:
            parent = Path(directory).resolve()
            private = parent / "private"
            parent.chmod(0o770)
            try:
                for mode in ("qualification", "focused"):
                    for scope in gate.QUALIFICATION_SCOPES:
                        with self.subTest(mode=mode, scope=scope), contextlib.ExitStack() as stack:
                            stack.enter_context(patch.dict(os.environ, {"TAIRA_TESTNET_BEACON_FIXTURE_DIR": str(private)}))
                            stack.enter_context(patch.object(gate, "require_native_artifact_inspector"))
                            actions = [stack.enter_context(patch.object(gate, name)) for name in (
                                "require_network_fixture_capacity", "run_pure_fsm_checks", "run_lifecycle_source_checks",
                                "shipping_harnesses", "check_test_harnesses", "compile_test_harnesses", "compile_network_binaries")]
                            head = stack.enter_context(patch.object(gate.subprocess, "check_output"))
                            with self.assertRaisesRegex(gate.CheckError, "ancestor.*group or world write") as failed:
                                self.invoke(mode, scope)
                            self.assertIn(str(parent), str(failed.exception))
                            self.assertFalse(private.exists())
                            for action in (*actions, head):
                                action.assert_not_called()
            finally:
                parent.chmod(0o700)

    def test_shared_ancestors_fail_before_creating_private_leaf(self):
        with tempfile.TemporaryDirectory(dir=Path.home().resolve()) as directory:
            parent = Path(directory).resolve()
            intermediate = parent / "intermediate"
            intermediate.mkdir(mode=0o700)
            private = intermediate / "private"
            for mode in (0o770, 0o707, 0o1777):
                parent.chmod(mode)
                try:
                    with self.subTest(mode=oct(mode)), \
                         patch.dict(os.environ, {"TAIRA_TESTNET_BEACON_FIXTURE_DIR": str(private)}):
                        with self.assertRaisesRegex(gate.CheckError, "ancestor.*group or world write") as failed:
                            gate.beacon_fixture_root()
                        self.assertIn(str(parent), str(failed.exception))
                        self.assertFalse(private.exists())
                finally:
                    parent.chmod(0o700)

    def test_symlinked_ancestor_and_git_marker_fail_before_leaf_creation(self):
        with tempfile.TemporaryDirectory(dir=Path.home().resolve()) as directory:
            parent = Path(directory).resolve()
            actual = parent / "actual"
            actual.mkdir(mode=0o700)
            alias = parent / "alias"
            alias.symlink_to(actual, target_is_directory=True)
            with patch.dict(os.environ, {"TAIRA_TESTNET_BEACON_FIXTURE_DIR": str(alias / "private")}):
                with self.assertRaisesRegex(gate.CheckError, "absolute direct path"):
                    gate.beacon_fixture_root()
                self.assertFalse((actual / "private").exists())
            for marker_kind in ("file", "directory"):
                marker = parent / ".git"
                if marker_kind == "file":
                    marker.write_text("gitdir: fixture-only\n")
                else:
                    marker.mkdir(mode=0o700)
                try:
                    with self.subTest(marker_kind=marker_kind), \
                         patch.dict(os.environ, {"TAIRA_TESTNET_BEACON_FIXTURE_DIR": str(actual / "private")}), \
                         patch.object(gate.subprocess, "run") as discovery:
                        with self.assertRaisesRegex(gate.CheckError, "outside a Git") as failed:
                            gate.beacon_fixture_root()
                        self.assertIn(str(parent), str(failed.exception))
                        self.assertFalse((actual / "private").exists())
                        discovery.assert_not_called()
                finally:
                    marker.unlink() if marker_kind == "file" else marker.rmdir()


class NativeArtifactInspectorPrerequisiteTests(unittest.TestCase):
    env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}

    def invoke(self, mode, scope):
        if mode == "focused":
            gate.run_prequalification(
                Path("/unread"), qualification_scope=scope,
                focused_regressions=("cli=" + gate.STAGES[0][1][0],),
                environment=self.env, lock_fds=())
        else:
            gate.run_checks(Path("/unread"), qualification_scope=scope, environment=self.env)

    def test_platform_inspector_path_is_fixed_and_ignores_path_environment(self):
        for platform, expected in (("darwin", "/usr/sbin/lsof"), ("linux", "/usr/bin/lsof")):
            with self.subTest(platform=platform), patch.object(gate.sys, "platform", platform), \
                 patch.dict(os.environ, {"PATH": "/untrusted/bin"}):
                self.assertEqual(gate.native_artifact_inspector_path(), Path(expected))

    def test_missing_or_nonexecutable_inspector_stops_every_gate_before_source_or_compile(self):
        with tempfile.TemporaryDirectory() as directory:
            inspector = Path(directory) / "lsof"
            for state in ("missing", "nonexecutable"):
                if state == "nonexecutable":
                    inspector.write_bytes(b"fixture, never executed")
                    inspector.chmod(0o600)
                for platform in ("darwin", "linux"):
                    for mode in ("qualification", "focused"):
                        for scope in gate.QUALIFICATION_SCOPES:
                            with self.subTest(state=state, platform=platform, mode=mode, scope=scope), \
                                 contextlib.ExitStack() as stack:
                                stack.enter_context(patch.object(gate.sys, "platform", platform))
                                stack.enter_context(patch.object(gate, "native_artifact_inspector_path",
                                                                 return_value=inspector))
                                actions = [stack.enter_context(patch.object(gate, name)) for name in (
                                    "require_network_fixture_capacity", "run_pure_fsm_checks",
                                    "run_lifecycle_source_checks", "shipping_harnesses", "check_test_harnesses",
                                    "compile_test_harnesses", "compile_network_binaries", "run_network_checks")]
                                git = stack.enter_context(patch.object(gate.subprocess, "check_output"))
                                with self.assertRaises(gate.CheckError) as failed:
                                    self.invoke(mode, scope)
                                self.assertIn(str(inspector), str(failed.exception))
                                self.assertIn("install lsof", str(failed.exception))
                                self.assertIn("missing" if state == "missing" else "not executable",
                                              str(failed.exception))
                                for action in (*actions, git):
                                    action.assert_not_called()

    def test_executable_inspector_allows_every_gate_to_reach_first_compile_stage(self):
        with tempfile.TemporaryDirectory() as directory:
            inspector = Path(directory) / "lsof"
            inspector.write_bytes(b"fixture, never executed")
            inspector.chmod(0o700)
            for platform in ("darwin", "linux"):
                for mode in ("qualification", "focused"):
                    for scope in gate.QUALIFICATION_SCOPES:
                        with self.subTest(platform=platform, mode=mode, scope=scope), \
                             patch.object(gate.sys, "platform", platform), \
                             patch.object(gate, "native_artifact_inspector_path", return_value=inspector), \
                             patch.object(gate, "require_network_fixture_prerequisites"), \
                             patch.object(gate.subprocess, "check_output", return_value="a" * 40 + "\n"), \
                             patch.object(gate, "run_pure_fsm_checks",
                                          side_effect=RuntimeError("first compile reached")) as first_compile, \
                             contextlib.redirect_stdout(io.StringIO()):
                            with self.assertRaisesRegex(RuntimeError, "first compile reached"):
                                self.invoke(mode, scope)
                            first_compile.assert_called_once()
                            self.assertEqual(first_compile.call_args.args[0], Path("/unread"))

    def test_inspector_disappearance_or_execution_failure_still_retains_outputs(self):
        with patch.object(Path, "is_file", return_value=False), \
             patch.object(gate.subprocess, "run") as run:
            self.assertIsNone(gate.native_test_output_confirmed_closed(Path("/exact/test")))
            run.assert_not_called()
        with patch.object(Path, "is_file", return_value=True), \
             patch.object(gate.subprocess, "run", side_effect=PermissionError("inspection denied")):
            self.assertIsNone(gate.native_test_output_confirmed_closed(Path("/exact/test")))


class NativeTestOutputOpenFileTests(unittest.TestCase):
    def test_actual_open_descriptor_is_retained(self):
        executable = Path("/usr/sbin/lsof" if sys.platform == "darwin" else "/usr/bin/lsof")
        if not executable.is_file():
            self.skipTest("lsof is unavailable; production retains files")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory).resolve() / "closed-or-open-test"
            path.write_bytes(b"test executable metadata")
            self.assertTrue(gate.native_test_output_confirmed_closed(path))
            with path.open("rb"):
                self.assertFalse(gate.native_test_output_confirmed_closed(path))
            self.assertTrue(gate.native_test_output_confirmed_closed(path))

    def test_os_result_must_be_unambiguously_closed(self):
        from subprocess import CompletedProcess, TimeoutExpired
        with patch.object(Path, "is_file", return_value=True), patch.object(gate.subprocess, "run") as run:
            for code, stdout, stderr, expected in [
                (1, b"", b"", True), (0, b"p123\n", b"", False),
                (1, b"", b"partial inspection", None), (2, b"", b"", None),
            ]:
                run.return_value = CompletedProcess([], code, stdout, stderr)
                self.assertEqual(gate.native_test_output_confirmed_closed(Path("/exact/test")), expected)
            run.side_effect = TimeoutExpired("lsof", 5)
            self.assertFalse(gate.native_test_output_confirmed_closed(Path("/exact/test")))



if __name__ == "__main__":
    unittest.main()
