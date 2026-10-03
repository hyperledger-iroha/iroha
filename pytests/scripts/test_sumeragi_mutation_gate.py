"""Keep the strict Sumeragi mutation table tied to real hooks and named tests."""

from __future__ import annotations

import importlib.util
import os
import re
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "sumeragi_mutation_gate_under_test", ROOT / "scripts" / "sumeragi_mutation_gate.py"
)
assert SPEC is not None and SPEC.loader is not None
gate = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = gate
SPEC.loader.exec_module(gate)


def test_duplicate_mutation_id_cannot_select_a_different_kill_test():
    original = gate.MUTATIONS[0]
    duplicate = gate.m(original.id, "another rule", ["unrelated_test"])
    with pytest.raises(ValueError, match=f"duplicate mutation id {original.id}"):
        gate.index_mutations([original, duplicate])
    assert gate.index_mutations([original]) == {original.id: original}


def test_every_registered_mutation_has_a_current_source_switch_and_named_test():
    sources = "\n".join(path.read_text() for path in gate.CRATE_SRC.rglob("*.rs"))
    switches = set(re.findall(r'sumeragi_mutation\s*=\s*"([^"]+)"', sources))
    functions = set(re.findall(r"\bfn\s+(\w+)\s*\(", sources))
    registered = [mutation.id for mutation in gate.MUTATIONS]
    assert len(registered) == len(set(registered)), "duplicate mutation id hides a gate entry"
    assert set(registered) <= switches, "retired hooks must not masquerade as active mutations"
    for mutation in gate.MUTATIONS:
        assert mutation.tests, f"{mutation.id} has no named deterministic kill test"
        for test in mutation.tests:
            assert test.rsplit("::", 1)[-1] in functions, (mutation.id, test)


def test_actual_availability_boundaries_have_separate_strict_kill_tests():
    expected = {
        "MS20": "acquisition_rejects_corrupt_actual_row_before_counting_custody",
        "MS20b": "source_bound_storage_job_never_confuses_refusal_with_absence_or_rebinds_source",
        "MS20c": "signed_inconsistent_codeword_and_payload_commitments_are_rejected",
        "MS20d": "signed_inconsistent_codeword_and_payload_commitments_are_rejected",
        "MS20e": "stored_body_restoration_checks_actual_codeword_without_resigning_or_copying_payload",
        "MS35": "det_s35_relay_tamper_no_evidence",
    }
    for mutation, test in expected.items():
        assert test in gate.BY_ID[mutation].tests
        assert gate.has_switch(mutation)


@pytest.mark.parametrize("baseline,expected", [("pass", 0), ("fail", 1), ("error", 1)])
def test_baseline_failure_cannot_pass_when_every_mutant_is_killed(
    monkeypatch, tmp_path, baseline, expected
):
    monkeypatch.setattr(
        sys, "argv",
        ["sumeragi_mutation_gate.py", "--only", "MX1", "--strict", "--fast",
         "--target-dir", str(tmp_path)],
    )
    monkeypatch.setattr(
        gate, "evaluate_baseline",
        lambda *_: {"id": "baseline", "verdict": baseline},
    )
    monkeypatch.setattr(
        gate, "evaluate",
        lambda *_: {"id": "MX1", "verdict": "killed_by_test"},
    )
    assert gate.main() == expected


@pytest.mark.parametrize(
    "code,output,expected",
    [
        (-9, "", "execution-error"),
        (-9, "test tests::named ... FAILED\n", "execution-error"),
        (101, "test tests::named ... ok\nerror: test process crashed", "execution-error"),
        (101, "test tests::named ... FAILED\nerror: test process crashed", "execution-error"),
        (0, "test tests::named ... ignored\n", "missing-test"),
        (101, "test tests::other ... FAILED\n", "missing-test"),
        (0, "test tests::named ... ok\ntest result: ok. 1 passed; 0 failed;\n", "pass"),
        (101, "test tests::named ... FAILED\ntest result: FAILED. 0 passed; 1 failed;\n", "fail"),
        (None, "test tests::named ... ", "timeout"),
    ],
)
def test_only_executed_named_test_results_establish_a_mutation_kill(
    monkeypatch, tmp_path, code, output, expected
):
    monkeypatch.setattr(gate, "cargo_test", lambda *args: (code, output, 1.0))
    step = gate.run_step(
        SimpleNamespace(), tmp_path, "MS1", ["named"], None, 5,
        tmp_path / "named.log",
    )
    assert step.status == expected


@pytest.mark.parametrize("status", ["execution-error", "missing-test", "timeout"])
def test_failed_test_execution_cannot_be_reported_as_a_named_kill(
    monkeypatch, tmp_path, status
):
    mutation = gate.BY_ID["MS1"]
    monkeypatch.setattr(gate, "has_switch", lambda _: True)
    monkeypatch.setattr(gate, "build", lambda *args: gate.Step(status="pass"))
    monkeypatch.setattr(gate, "run_step", lambda *args: gate.Step(status=status))
    args = SimpleNamespace(target_dir=tmp_path, timeout_test=5, fast=True)
    result = gate.evaluate(args, tmp_path, mutation)
    assert result["verdict"] == "error"


def test_missing_second_filter_is_an_error_even_if_first_named_test_fails(monkeypatch, tmp_path):
    monkeypatch.setattr(
        gate, "cargo_test", lambda *args: (101, "test tests::present ... FAILED\n", 1.0)
    )
    step = gate.run_step(
        SimpleNamespace(), tmp_path, "MS1", ["present", "missing"], None, 5,
        tmp_path / "named.log",
    )
    assert step.status == "missing-test"
    assert step.failed == ["tests::present"]


@pytest.mark.parametrize("timeout,expected", [(0, None), (20, 20)])
def test_explicit_unbounded_wait_never_installs_a_process_deadline(
    monkeypatch, tmp_path, timeout, expected
):
    deadlines = []

    class Process:
        returncode = 0

        def communicate(self, *, timeout):
            deadlines.append(timeout)
            return "test tests::named ... ok\n", None

    monkeypatch.setattr(gate.subprocess, "Popen", lambda *args, **kwargs: Process())
    code, output, _ = gate.cargo_test(
        SimpleNamespace(), tmp_path, "MS1", ["named"], None, timeout,
        tmp_path / "named.log",
    )
    assert deadlines == [expected]
    assert code == 0
    assert "tests::named ... ok" in output


@pytest.mark.parametrize("option", ["--timeout-build", "--timeout-test", "--timeout-scenario"])
def test_negative_timeouts_are_rejected_before_running_commands(monkeypatch, option):
    monkeypatch.setattr(sys, "argv", ["sumeragi_mutation_gate.py", option, "-1", "--list"])
    with pytest.raises(SystemExit) as error:
        gate.main()
    assert error.value.code == 2


def test_scenario_timeout_is_an_execution_error_even_when_named_test_was_killed(monkeypatch, tmp_path):
    monkeypatch.setattr(gate, "has_switch", lambda _: True)
    monkeypatch.setattr(gate, "build", lambda *args: gate.Step(status="pass"))
    outcomes = iter([
        gate.Step(status="fail", failed=[gate.BY_ID["MS1"].tests[0]]),
        gate.Step(status="timeout"),
    ])
    monkeypatch.setattr(gate, "run_step", lambda *args: next(outcomes))
    args = SimpleNamespace(
        target_dir=tmp_path, timeout_test=5, timeout_scenario=5, seeds=1, fast=False,
    )
    result = gate.evaluate(args, tmp_path, gate.BY_ID["MS1"])
    assert result["verdict"] == "error"
    assert result["reason"] == "scenarios: timeout"


@pytest.mark.parametrize("core", [False, True])
def test_package_mutation_environment_is_confined_to_its_actual_owner(monkeypatch, tmp_path, core):
    captured = {}
    class Process:
        returncode = 0
        def communicate(self, timeout):
            captured["timeout"] = timeout
            return "test result: ok. 1 passed; 0 failed;", None
    def popen(command, **options):
        captured["command"] = command
        captured["env"] = options["env"]
        return Process()
    monkeypatch.setenv("SUMERAGI_MUTATION", "foreign-protocol-hook")
    monkeypatch.setenv("SUMERAGI_CORE_MUTATION", "foreign-core-hook")
    monkeypatch.setattr(gate.subprocess, "Popen", popen)
    args = SimpleNamespace(core=core)
    mutation = "HC1" if core else "MS1"
    code, _, _ = gate.cargo_test(args, tmp_path, mutation, ["named"], None, 0, tmp_path / "log")
    crate, features, environment = gate.package_options(args)
    assert code == 0
    assert captured["command"][:9] == ["cargo", "test", "-p", crate, "--release", "--features", features, "--lib", "--"]
    assert captured["env"][environment] == mutation
    foreign = "SUMERAGI_MUTATION" if core else "SUMERAGI_CORE_MUTATION"
    assert foreign not in captured["env"]
    assert captured["timeout"] is None


def test_core_table_is_separate_from_protocol_scenario_hooks(monkeypatch, capsys):
    monkeypatch.setattr(sys, "argv", ["sumeragi_mutation_gate.py", "--core", "--list"])
    assert gate.main() == 0
    listing = capsys.readouterr().out
    assert "HC1" in listing
    assert "MS1" not in listing
    assert "f01" not in listing


@pytest.mark.parametrize("arguments", [["--core", "--only", "MS1"], ["--only", "HC1"]])
def test_mutation_ids_cannot_select_a_different_implementation_owner(monkeypatch, arguments):
    monkeypatch.setattr(sys, "argv", ["sumeragi_mutation_gate.py", *arguments])
    with pytest.raises(SystemExit) as error:
        gate.main()
    assert error.value.code == 2


def test_registered_core_rules_have_real_hooks_and_named_core_regressions():
    source = gate.REPO / "crates" / "iroha_core" / "src"
    text = "\n".join(path.read_text() for path in source.rglob("*.rs"))
    functions = set(re.findall(r"\bfn\s+(\w+)\s*\(", text))
    ids = [mutation.id for mutation in gate.CORE_MUTATIONS]
    assert len(ids) == len(set(ids))
    for mutation in gate.CORE_MUTATIONS:
        assert gate.has_switch(mutation.id, core=True)
        assert not gate.has_switch(mutation.id)
        assert mutation.tests and not mutation.scenarios
        assert all(name.rsplit("::", 1)[-1] in functions for name in mutation.tests)


@pytest.mark.parametrize("test_build,mutation_feature,accepted", [
    (False, False, True), (True, False, True),
    (True, True, True), (False, True, False),
])
def test_core_guard_rejects_mutation_feature_in_non_test_builds(
    tmp_path, test_build, mutation_feature, accepted
):
    guard = ROOT / "crates/iroha_core/src/mutation_guard.rs"
    assert "mod mutation_guard;" in (guard.parent / "lib.rs").read_text()
    command = ["rustc", "--edition=2024", "--crate-type=lib", "--emit=metadata",
               str(guard), "-o", str(tmp_path / "guard.rmeta")]
    if test_build:
        command += ["--cfg", "test"]
    if mutation_feature:
        command += ["--cfg", 'feature="mutation-testing"']
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    assert (result.returncode == 0) == accepted, result.stderr
    if not accepted:
        assert "mutation-testing is test-only" in result.stderr


@pytest.mark.parametrize("status", ["execution-error", "missing-test", "timeout"])
def test_core_gate_preserves_strict_failed_execution_classification(monkeypatch, tmp_path, status):
    monkeypatch.setattr(gate, "has_switch", lambda _, *, core: core)
    monkeypatch.setattr(gate, "build", lambda *args: gate.Step(status="pass"))
    monkeypatch.setattr(gate, "run_step", lambda *args: gate.Step(status=status))
    args = SimpleNamespace(core=True, target_dir=tmp_path, timeout_test=0, fast=True)
    result = gate.evaluate(args, tmp_path, gate.CORE_MUTATIONS[0])
    assert result["verdict"] == "error"
    assert result["reason"] == f"named tests: {status}"


@pytest.fixture(scope="module")
def core_build_script(tmp_path_factory):
    executable = tmp_path_factory.mktemp("core-mutation-build") / "build-script"
    result = subprocess.run(
        ["rustc", "--edition=2024", str(ROOT / "crates/iroha_core/build.rs"),
         "-o", str(executable)], capture_output=True, text=True, check=False,
    )
    assert result.returncode == 0, result.stderr
    return executable


@pytest.mark.parametrize("feature,mutation,flags,accepted,emitted", [
    (False, "HC1", "", True, False),
    (False, None, '--cfg\x1fsumeragi_core_mutation="HC1"', False, False),
    (True, "HC1", "", True, True),
    (True, None, "", True, False),
    (True, "unknown_rule", "", False, False),
    (True, 'HC1"', "", False, False),
])
def test_core_build_script_never_treats_environment_as_a_production_fault_switch(
    core_build_script, feature, mutation, flags, accepted, emitted
):
    environment = dict(os.environ)
    for key in ("CARGO_FEATURE_MUTATION_TESTING", "SUMERAGI_CORE_MUTATION"):
        environment.pop(key, None)
    environment["SUMERAGI_MUTATION"] = "foreign-protocol-rule"
    environment["CARGO_ENCODED_RUSTFLAGS"] = flags
    if feature:
        environment["CARGO_FEATURE_MUTATION_TESTING"] = "1"
    if mutation is not None:
        environment["SUMERAGI_CORE_MUTATION"] = mutation
    result = subprocess.run(
        [str(core_build_script)], cwd=ROOT / "crates/iroha_core", env=environment,
        capture_output=True, text=True, check=False,
    )
    assert (result.returncode == 0) == accepted, result.stderr
    assert ('cargo:rustc-cfg=sumeragi_core_mutation="HC1"' in result.stdout) == emitted
    assert "cargo:rustc-cfg=sumeragi_mutation=" not in result.stdout
    if not feature and mutation:
        assert "ignored" in result.stdout


@pytest.mark.parametrize("arguments", [
    ["--core-profile", "test"], ["--core", "--core-profile", "dev"],
    ["--core", "--core-profile", "bogus"], ["--core", "--core-profile", ""],
])
def test_core_profile_rejects_invalid_values_and_protocol_overrides(monkeypatch, arguments):
    monkeypatch.setattr(sys, "argv", ["sumeragi_mutation_gate.py", *arguments, "--list"])
    with pytest.raises(SystemExit) as error:
        gate.main()
    assert error.value.code == 2


@pytest.mark.parametrize("profile", ["release", "test"])
def test_core_profile_is_identical_for_baseline_and_mutant_and_recorded(monkeypatch, tmp_path, profile):
    captured = []
    class Process:
        returncode = 0
        def communicate(self, timeout):
            assert timeout is None
            return "test result: ok. 1 passed; 0 failed;", None
    def popen(command, **options):
        captured.append(command)
        return Process()
    monkeypatch.setattr(gate.subprocess, "Popen", popen)
    args = SimpleNamespace(core=True, core_profile=profile)
    for mutation in (None, "HC1"):
        assert gate.cargo_test(args, tmp_path, mutation, ["named"], None, 0,
                               tmp_path / f"{mutation}.log")[0] == 0
    assert captured[0] == captured[1]
    assert captured[0][4:6] == ["--profile", profile]
    monkeypatch.setattr(sys, "argv", ["sumeragi_mutation_gate.py", "--core", "--core-profile", profile,
                                     "--only", "HC1", "--strict", "--fast", "--target-dir", str(tmp_path)])
    monkeypatch.setattr(gate, "evaluate_baseline", lambda *_: {"id": "baseline", "verdict": "pass"})
    monkeypatch.setattr(gate, "evaluate", lambda *_: {"id": "HC1", "verdict": "killed_by_test"})
    assert gate.main() == 0
    import json
    assert json.loads((tmp_path / "report.json").read_text())["profile"] == profile


def test_hc10_selects_only_the_original_query_scratch_owner():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC10"]
    assert rule.tests == (
        "sumeragi::certified_chain::tests::state_certificate::"
        "state_certificate_signed_availability_scratch_uses_original_query_allowance",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC10", core=True)
    assert not gate.has_switch("HC10")


def test_hc12_selects_only_the_original_beacon_scratch_owner():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC12"]
    assert rule.tests == (
        "beacon::validation::tests::"
        "beacon_verification_reserves_exact_buffers_and_refuses_before_unfunded_work",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC12", core=True)
    assert not gate.has_switch("HC12")


def test_hc13_selects_only_the_original_query_pairing_constructor_owner():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC13"]
    assert rule.tests == (
        "sumeragi::certified_chain::tests::state_certificate::"
        "state_certificate_pairing_constructor_refusal_preserves_original_source_for_retry",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC13", core=True)
    assert not gate.has_switch("HC13")


def test_hc14_selects_only_the_original_result_witness_binding():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC14"]
    assert rule.tests == (
        "sumeragi::certified_chain::artifacts::tests::"
        "original_result_witness_rejects_foreign_canonical_bytes_before_borrowing_graph",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC14", core=True)
    assert not gate.has_switch("HC14")


def test_hc15_selects_only_the_original_committed_quorum_owner():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC15"]
    assert rule.tests == (
        "sumeragi::block_store::committed_read::tests::"
        "committed_read_returns_original_qc_backing_after_projection_refusal_and_retry",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC15", core=True)
    assert not gate.has_switch("HC15")


def test_hc16_selects_only_original_lane_signer_ownership_handoffs():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC16"]
    assert rule.tests == (
        "sumeragi::lanes::custody::tests::original_signer_state_handoff_retains_backing_and_refuses_foreign_pool",
        "sumeragi::lanes::custody::tests::original_signer_world_handoff_admits_both_generations_before_replacing_either",
        "state::deserialize::native_lane_custody_tests::native_lane_signer_snapshot_retains_exact_raw_source_until_both_cuts_are_funded",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC16", core=True)
    assert not gate.has_switch("HC16")


def test_hc17_selects_only_current_tip_control_publication_binding():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC17"]
    assert rule.tests == (
        "sumeragi::epoch_beacon::producer::tests::"
        "control_requires_original_tip_and_matching_published_hash_journal",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC17", core=True)
    assert not gate.has_switch("HC17")


def test_hc19_preserves_amx_decoder_refusal_outside_instruction_results():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC19"]
    assert rule.tests == (
        "sumeragi::amx::tests::amx_anchor_decode_refusal_cannot_publish_even_when_instruction_error_is_caught",
        "sumeragi::amx::tests::amx_relay_decode_refusal_keeps_original_undecided_record_and_retries_proof",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC19", core=True)
    assert not gate.has_switch("HC19")


def test_hc18_selects_only_original_lane_sample_ownership_handoffs():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC18"]
    assert rule.tests == (
        "sumeragi::lanes::custody::tests::sample_state_admission_refuses_unfunded_source",
        "sumeragi::lanes::step::sample_owner_tests::sample_finalizer_refusal_preserves_exact_source_and_retry_funds_only_suffix",
        "state::deserialize::native_lane_custody_tests::native_lane_sample_snapshot_retains_raw_source_through_both_cut_refusal_and_retry",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC18", core=True)
    assert not gate.has_switch("HC18")


def test_hc20_preserves_original_stored_result_read_on_decoder_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC20"]
    assert rule.tests == (
        "sumeragi::block_store::body_read::tests::stored_result_decode_refusal_retains_original_decoded_owners_and_retries",
        "sumeragi::block_store::committed_read::tests::committed_result_decode_refusal_keeps_original_read_slot_and_retries",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC20", core=True)
    assert not gate.has_switch("HC20")


def test_hc21_selects_only_original_root_store_affinity():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC21"]
    assert rule.tests == (
        "sumeragi::node::tests::root_owner_tests::prepared_root_uses_original_state_store",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC21", core=True)
    assert not gate.has_switch("HC21")


def test_hc22_checks_every_canonical_queue_entrypoint_domain():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC22"]
    assert rule.tests == (
        "queue::tests::queue_rejects_preaccepted_foreign_external_before_custody",
        "queue::tests::queue_rejects_preaccepted_foreign_commitment_before_custody",
        "queue::tests::queue_rejects_preaccepted_foreign_reveal_before_custody",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC22", core=True)
    assert not gate.has_switch("HC22")


def test_hc24_preserves_allocation_free_original_read_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC24"]
    assert rule.tests == (
        "sumeragi::block_store::body_read::tests::stored_result_decode_refusal_retains_original_decoded_owners_and_retries",
        "sumeragi::block_store::committed_read::tests::committed_result_decode_refusal_keeps_original_read_slot_and_retries",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC24", core=True)
    assert not gate.has_switch("HC24")


def test_hc23_requires_retained_actor_before_startup_effects():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC23"]
    assert rule.tests == (
        "sumeragi::node::tests::p2p_owner_tests::network_start_rejects_closed_retained_actor_before_driver_files",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC23", core=True)
    assert not gate.has_switch("HC23")


def test_hc25_preserves_physical_certificate_decode_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC25"]
    assert rule.tests == (
        "sumeragi::block_store::body_read::tests::stored_certificate_allocator_refusal_keeps_original_read_and_retries",
        "sumeragi::block_store::committed_read::tests::committed_certificate_allocator_refusal_retains_original_slot_and_retries",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC25", core=True)
    assert not gate.has_switch("HC25")


def test_core_amx_role_gate_names_signed_genesis_and_all_coordinator_operations():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC26"]
    assert rule.tests == (
        "sumeragi::node::tests::dataspace_roots::amx_scope_tests::signed_private_genesis_cannot_install_global_amx_coordinator",
        "executor::root_scope::tests::amx_roles::every_amx_coordinator_instruction_rejects_private_execution_before_proof_decoding",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC26", core=True)
    assert not gate.has_switch("HC26")


def test_core_root_scope_refusal_gate_requires_original_signed_retry():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC27"]
    assert rule.tests == (
        "sumeragi::node::tests::dataspace_roots::scope_refusal_tests::signed_private_work_keeps_scope_decode_refusal_local_and_retries_original_carrier",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC27", core=True)
    assert not gate.has_switch("HC27")


def test_core_readonly_scope_gate_requires_query_and_vm_host_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC28"]
    assert rule.tests == (
        "sumeragi::node::tests::dataspace_roots::scope_refusal_tests::signed_private_manifest_query_does_not_turn_scope_refusal_into_permanent_error",
        "sumeragi::node::tests::dataspace_roots::scope_refusal_tests::signed_private_host_query_keeps_scope_refusal_out_of_completed_vm_errors",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC28", core=True)
    assert not gate.has_switch("HC28")


def test_hc29_keeps_original_lane_batch_refusal_local_through_read_execute_and_recovery():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC29"]
    assert rule.tests == (
        "sumeragi::lanes::tests::original_signed_batch_decode_preserves_exact_local_refusal_and_terminal_limits",
        "sumeragi::lanes::registry::tests::authenticated_registry_batch_decode_refusal_is_retryable_not_byzantine",
        "sumeragi::lanes::executor::native_decode_tests::signed_four_validator_lane_execution_refusal_never_caches_invalid_or_publishes",
        "sumeragi::lanes::executor::native_decode_tests::signed_four_validator_lane_recovery_keeps_available_phase_and_exact_original_owners",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC29", core=True)
    assert not gate.has_switch("HC29")


def test_hc31_borrows_original_lane_payload_and_checks_exact_allocation_census():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC31"]
    assert rule.tests == (
        "sumeragi::lanes::tests::signed_lane_batch_canonical_validation_preserves_original_bytes_without_new_allocations",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC31", core=True)
    assert not gate.has_switch("HC31")


def test_core_registry_refusal_gate_requires_native_fee_host_and_parliament_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC30"]
    assert rule.tests == (
        'sumeragi::node::tests::dataspace_roots::scope_refusal_tests::signed_private_contract_lookup_does_not_turn_scope_refusal_into_vm_permission_denial',
        'sumeragi::node::tests::dataspace_roots::scope_refusal_tests::signed_private_account_permission_read_defers_without_constructing_a_json_token',
        'validation_fee::tests::signed_fee_runtime_read_does_not_turn_scope_refusal_into_a_nonmatching_origin',
        'validation_fee::tests::original_retained_fee_registry_does_not_publish_local_decode_refusal_as_malformed',
        'smartcontracts::isi::world::isi::tests::signed_payout_scope_refusal_cannot_publish_a_parliament_terminal_outcome',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC30", core=True)
    assert not gate.has_switch("HC30")


def test_core_credit_reader_gate_requires_record_and_asset_original_source_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC32"]
    assert rule.tests == (
        "validation_fee::tests::original_treasury_credit_record_decode_refusal_preserves_balance_and_retries",
        "validation_fee::tests::original_treasury_credit_asset_decode_refusal_preserves_binding_and_retries",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC32", core=True)
    assert not gate.has_switch("HC32")


def test_core_decode_refusal_classifier_gate_retains_global_and_inner_format_rejections():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC33"]
    assert rule.tests == (
        "execution_attempt::tests::norito_global_archive_cap_is_terminal_inside_an_outer_decode_scope",
        "execution_attempt::tests::norito_inner_format_limits_are_terminal_under_a_wider_outer_scope",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC33", core=True)
    assert not gate.has_switch("HC33")


def test_core_sns_permission_gate_requires_original_record_refusal_and_retry_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC35"]
    assert rule.tests == (
        "executor::tests::original_sns_alias_domain_permission_refusal_retries_without_a_rejection",
        "executor::tests::original_sns_domain_transfer_permission_refusal_retries_without_a_rejection",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC35", core=True)
    assert not gate.has_switch("HC35")


def test_core_claim_fee_gate_requires_original_metadata_and_alias_refusal_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC36"]
    assert rule.tests == (
        "executor::tests::original_claim_metadata_refusal_defers_fee_quote_and_retries_exact_payload",
        "executor::tests::original_claim_alias_refusal_after_metadata_defers_fee_quote_and_retries",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC36", core=True)
    assert not gate.has_switch("HC36")


def test_core_fee_selector_gate_requires_original_account_and_currency_read_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC37"]
    assert rule.tests == (
        "block::original_canonical_account_refusal_does_not_fall_through_to_alias_absence",
        "executor::tests::original_network_xor_pin_refusal_defers_quote_and_retries_same_parameter",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC37", core=True)
    assert not gate.has_switch("HC37")
def test_native_source_owner_requires_applied_transcripts_and_isolated_capacity():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC34"]
    assert rule.tests == (
        'fastpq::source_reservation::admission::tests::native_authorization_has_no_entry_until_an_applied_transcript_and_drops_atomically',
        'fastpq::source_reservation::admission::tests::native_pool_overflow_cannot_borrow_ordinary_or_governance_reservations',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC34", core=True)
    assert not gate.has_switch("HC34")




def test_core_routing_gate_requires_original_sns_custody_and_capture_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC38"]
    assert rule.tests == (
        "queue::router::tests::original_dataspace_alias_read_refusal_keeps_routing_retryable",
        "queue::router::tests::original_physical_policy_read_refusal_never_enters_captured_row",
        "executor::root_scope::tests::original_private_instruction_routing_refusal_latches_before_scope_verdict",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC38", core=True)
    assert not gate.has_switch("HC38")


def test_core_native_metadata_gate_requires_original_root_and_late_policy_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC39"]
    assert rule.tests == (
        "state::network_policy_routes::tests::original_root_scope_read_refusal_does_not_publish_invalid_native_context",
        "state::network_policy_routes::tests::original_lane_policy_read_refusal_after_root_keeps_exact_capture_retryable",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC39", core=True)
    assert not gate.has_switch("HC39")

def test_account_matchers_retain_original_sns_and_canonical_read_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC40"]
    assert rule.tests == ('queue::router::tests::account_refusal_tests::original_account_target_refusal_never_selects_the_default_route', 'queue::router::tests::account_refusal_tests::original_canonical_account_matcher_preserves_decode_refusal_and_retry', 'queue::router::tests::account_refusal_tests::original_signed_account_matcher_refusal_is_not_a_mismatch_or_panic', 'queue::router::tests::account_refusal_tests::original_native_policy_account_matcher_retains_refusal_before_route_selection')
    assert not rule.scenarios
    assert gate.has_switch("HC40", core=True)
    assert not gate.has_switch("HC40")


def test_parameter_control_scope_gate_preserves_signed_global_and_physical_authority():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC41"]
    assert rule.tests == (
        "queue::router::alias_registry_routing_tests::parameter_control_preserves_global_physical_route_before_private_account_rule",
        "queue::router::alias_registry_routing_tests::alias_registry_routing_paid_post_genesis_dataspace_domain_and_renewal",
        "queue::router::alias_registry_routing_tests::alias_registry_routing_cold_replay_with_expanded_catalog_preserves_paid_bootstrap",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC41", core=True)
    assert not gate.has_switch("HC41")

def test_auto_renew_rekey_keeps_current_owner_revision_and_capacity():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC42"]
    assert rule.tests == ('smartcontracts::isi::sns::rekey_auto_renew_tests::signed_rekey_current_owner_can_replace_stale_auto_renew_configuration', 'smartcontracts::isi::sns::rekey_auto_renew_tests::signed_rekey_same_configuration_requires_owner_replacement_cas', 'smartcontracts::isi::sns::rekey_auto_renew_tests::signed_rekey_disabled_clean_record_requires_exact_owner_revision')
    assert not rule.scenarios
    assert gate.has_switch("HC42", core=True)
    assert not gate.has_switch("HC42")


def test_worker_validation_and_certificate_gate_preserves_original_attempt_owner():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC44"]
    assert rule.tests == (
        "sumeragi::executor::validation_refusal_tests::original_post_merge_validation_refusal_retains_worker_owner_and_exact_available_retry",
        "sumeragi::executor::validation_refusal_tests::original_prepared_certificate_read_refusal_retains_worker_owner_and_funded_execution",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC44", core=True)
    assert not gate.has_switch("HC44")

def test_payload_local_decoder_gate_retains_original_typed_worker_reason():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC45"]
    assert rule.tests == ("sumeragi::executor::payload_refusal_tests::original_available_payload_local_decode_refusal_keeps_typed_worker_reason_and_retry",)
    assert not rule.scenarios
    assert gate.has_switch("HC45", core=True)
    assert not gate.has_switch("HC45")


def test_narrow_caller_depth_gate_requires_original_bytes_and_worker_retry():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC46"]
    assert rule.tests == (
        "execution_attempt::tests::original_surviving_narrow_decode_depth_refusal_retries_identical_bytes",
        "sumeragi::executor::payload_refusal_tests::original_available_payload_narrow_depth_refusal_keeps_typed_worker_reason_and_retry",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC46", core=True)
    assert not gate.has_switch("HC46")

def test_certified_history_keeps_original_result_and_successor_refusal_local():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC43"]
    assert rule.tests == ('sumeragi::certified_chain::refusal_tests::original_result_frame_refusal_is_local_and_same_bytes_retry', 'sumeragi::certified_chain::refusal_tests::original_successor_history_refusal_is_local_and_same_source_retries', 'sumeragi::certified_chain::tests::state_certificate::state_certificate_native_qc_decode_refusal_is_capacity_and_retries_original_source')
    assert not rule.scenarios
    assert gate.has_switch("HC43", core=True)
    assert not gate.has_switch("HC43")
def test_staking_payload_gate_preserves_original_preparation_and_worker_owners():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC49"]
    assert rule.tests == (
        "sumeragi::penalties::tests::original_staking_payload_refusal_retains_evidence_pool_and_exact_assembly_retry",
        "sumeragi::executor::publication_tests::original_staking_payload_worker_retains_pool_refusal_and_exact_queued_retry",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC49", core=True)
    assert not gate.has_switch("HC49")




def test_availability_attempts_keep_original_reader_and_retry_owners():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC47"]
    assert rule.tests == ('sumeragi::certified_chain::refusal_tests::original_availability_history_refusal_is_pending_without_corruption', 'sumeragi::certified_chain::refusal_tests::original_availability_constructor_refusal_retries_without_installing_authority', 'sumeragi::driver::exec::refusal_tests::append_refusal_keeps_original_commit_and_release_owner_until_durable', 'sumeragi::driver::serve::tests::refused_metadata_owner_cannot_be_replaced_by_another_peer_during_backoff')
    assert not rule.scenarios
    assert gate.has_switch("HC47", core=True)
    assert not gate.has_switch("HC47")


def test_lane_history_attempts_preserve_original_producers_and_consumer_owners():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC51"]
    assert rule.tests == ('sumeragi::runtime_availability::history::source_refusal_tests::original_archive_read_refusal_preserves_pool_release_and_same_lane_prefix', 'sumeragi::runtime_availability::history::source_refusal_tests::original_certificate_projection_refusal_preserves_pool_release_and_exact_carrier', 'sumeragi::runtime_availability::history::source_refusal_tests::original_lane_evidence_handoff_preserves_actual_decode_refusal_and_exact_cut', 'sumeragi::lanes::registry::tests::original_native_lane_authority_refusal_reaches_merge_and_original_pool_retry', 'sumeragi::evidence::tests::original_lane_history_refusal_reaches_evidence_without_recovery_or_rejection', 'sumeragi::executor::publication_tests::original_lane_policy_proposal_refusal_retains_worker_owner_and_exact_queued_retry')
    assert not rule.scenarios
    assert gate.has_switch("HC51", core=True)
    assert not gate.has_switch("HC51")


def test_incumbent_and_lifecycle_refusals_keep_original_state_attempt():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC48"]
    assert rule.tests == ('state::validator_committee::tests::refusal::original_incumbent_history_refusal_keeps_authority_and_same_source_retry', 'state::validator_committee::tests::refusal::original_candidate_authority_refusal_keeps_command_and_same_source_retry', 'state::validator_committee::tests::refusal::original_candidate_command_decode_refusal_has_no_publication_and_retries', 'state::validator_committee::tests::refusal::original_beacon_public_state_decode_refusal_defers_before_installation', 'state::validator_committee::tests::refusal::original_tle_public_state_decode_refusal_defers_before_installation', 'state::validator_committee::tests::refusal::original_staking_authority_refusal_keeps_exit_overlay_and_same_signed_retry')
    assert not rule.scenarios
    assert gate.has_switch("HC48", core=True)
    assert not gate.has_switch("HC48")


def test_original_npos_policy_read_refusal_keeps_connected_owners():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC50"]
    assert rule.tests == ('state::validator_committee::tests::refusal::original_npos_parameter_refusal_does_not_become_missing_staking_policy', 'state::validator_committee::tests::refusal::original_npos_exit_policy_refusal_keeps_stake_and_same_signed_retry', 'state::validator_committee::tests::refusal::original_npos_reserve_validation_refuses_without_changing_current_or_undo', 'smartcontracts::ivm::host::return_resource_tests::original_npos_policy_refusal_preserves_host_seed_projection_and_retries', 'sumeragi::evidence::tests::original_npos_policy_refusal_cannot_prune_retained_evidence', 'state::validator_committee::tests::refusal::late_original_npos_activation_read_refusal_rolls_back_and_same_signed_retry', 'sumeragi::evidence_history::lane::tests::original_lane_observer_late_policy_refusal_retains_observation_and_retries')
    assert not rule.scenarios
    assert gate.has_switch("HC50", core=True)
    assert not gate.has_switch("HC50")


def test_original_checkpoint_decode_resource_stays_a_typed_attempt():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC52"]
    assert rule.tests == ('sumeragi::finality::tests::original_checkpoint_binary_refusal_is_local_and_retries_exact_original_source',)
    assert not rule.scenarios
    assert gate.has_switch("HC52", core=True)
    assert not gate.has_switch("HC52")


@pytest.mark.parametrize("late_verdict,late_exit", [("ok", 0), ("FAILED", 101)])
def test_deadline_waits_for_owned_cargo_without_signalling_or_counting_late_result(
    monkeypatch, tmp_path, late_verdict, late_exit
):
    late_output = (
        f"test tests::named ... {late_verdict}\n"
        + ("test result: ok. 1 passed; 0 failed;\n" if late_exit == 0
           else "test result: FAILED. 0 passed; 1 failed;\n")
    )
    calls = []
    omitted = object()

    class Process:
        pid = 123
        returncode = late_exit

        def communicate(self, *, timeout=omitted):
            calls.append(timeout)
            if timeout is not omitted:
                raise subprocess.TimeoutExpired("cargo test", timeout)
            return late_output, None

    monkeypatch.setattr(gate.subprocess, "Popen", lambda *args, **kwargs: Process())
    monkeypatch.setattr(
        gate.os, "killpg",
        lambda *args: pytest.fail("a qualification deadline must not signal Cargo"),
    )
    args = SimpleNamespace(core=True, core_profile="test")
    log = tmp_path / "owned-deadline.log"
    code, output, _ = gate.cargo_test(args, tmp_path, None, ["named"], None, 7, log)
    assert calls == [7, omitted], "original deadline is retained; then wait for natural exit"
    assert code is None, "neither late success nor a late failed test satisfies its deadline"
    assert output == late_output
    assert "# exit None" in log.read_text()
    monkeypatch.setattr(gate, "cargo_test", lambda *args: (code, output, 8))
    step = gate.run_step(args, tmp_path, None, ["named"], None, 7, log)
    assert step.status == "timeout"
    assert step.ran == ["tests::named"]


def test_suspend_inclusive_network_time_has_original_deterministic_kill_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC53"]
    assert rule.tests == (
        'time::tests::suspend_inclusive_clock_advances_admission_and_expires_retained_probes',
        'time::tests::suspend_inclusive_clock_counts_entire_probe_round_trip',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC53", core=True)
    assert not gate.has_switch("HC53")


def test_beacon_generation_binding_has_genuine_dkg_kill_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC54"]
    assert rule.tests == (
        'state::validator_committee::tests::generation::committee_bootstrap_rejects_genuine_dkg_from_another_generation',
        'state::validator_committee::tests::generation::committee_restore_rejects_genuine_dkg_from_another_generation',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC54", core=True)
    assert not gate.has_switch("HC54")


def test_fee_reward_claim_has_exact_signed_entitlement_kill_control():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC55"]
    assert rule.tests == (
        'validation_fee_rewards::tests::signed_fee_reward_claim_rejects_every_changed_binding_before_mutation',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC55", core=True)
    assert not gate.has_switch("HC55")


def test_shared_staking_custody_has_additive_fee_reserve_kill_control():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC56"]
    assert rule.tests == (
        'validation_fee_rewards::tests::shared_fee_stake_reward_custody_is_additive',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC56", core=True)
    assert not gate.has_switch("HC56")


def test_completed_replay_requires_exact_certificate_and_source_kill_control():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC57"]
    assert rule.tests == (
        'sumeragi::executor::replay::tests::completed_replay_rejects_altered_certificate_and_source_without_losing_exact_retry',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC57", core=True)
    assert not gate.has_switch("HC57")


def test_fee_rewards_require_canonical_network_xor_kill_control():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC58"]
    assert rule.tests == (
        'validation_fee_rewards::tests::canonical_network_xor_is_required_before_fee_reward_state_changes',
        'state::network_xor::tests::network_xor_rejects_wrong_definition_scope_and_precision',
        'smartcontracts::isi::staking::tests::staking_registration_rejects_wrong_xor_shape_with_transaction_rollback',
        'smartcontracts::isi::staking::tests::reward_claim_rejects_wrong_xor_shape_with_transaction_rollback',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC58", core=True)
    assert not gate.has_switch("HC58")
    assert 'sumeragi_core_mutation = "HC58"' in (
        gate.REPO / "crates/iroha_core/src/state/network_xor.rs"
    ).read_text()


def test_inline_beacon_reducer_verifies_each_share_before_retention():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC59"]
    assert rule.tests == (
        'beacon::tests::threshold_beacon_inline_reducer_rejects_invalid_share_without_losing_original_slots',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC59", core=True)
    assert not gate.has_switch("HC59")


def test_preparation_capacity_source_survives_queued_publication_retry():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC60"]
    assert rule.tests == (
        'sumeragi::executor::preparation::tests::certificate_capacity_refusal_reaches_scheduler_with_original_release_and_execution',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC60", core=True)
    assert not gate.has_switch("HC60")


def test_lane_anchor_history_preserves_original_refusal_and_same_source_retry():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC61"]
    assert rule.tests == (
        'sumeragi::lanes::executor::native_decode_tests::anchor_read_refusal_retains_original_lane_body_until_capacity_returns',
        'sumeragi::lanes::evidence::tests::anchor_history_refusal_retains_exact_source_through_lane_evidence_retry',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC61", core=True)
    assert not gate.has_switch("HC61")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/lanes/mod.rs"
    assert 'sumeragi_core_mutation = "HC61"' in source.read_text()


def test_prepared_block_keeps_original_shared_execution_control():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC62"]
    assert rule.tests == (
        'sumeragi::executor::publication_tests::prepared_block_moves_original_graph_and_rejects_replaced_shared_control',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC62", core=True)
    assert not gate.has_switch("HC62")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/executor.rs"
    assert 'sumeragi_core_mutation = "HC62"' in source.read_text()


def test_state_publication_preserves_original_resource_owners():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC63"]
    assert rule.tests == (
        'sumeragi::executor::publication::tests::state_publication_lock_refusals_reach_scheduler_with_original_execution_and_release',
        'sumeragi::executor::publication::tests::state_execution_and_membership_refusals_preserve_actual_release_owners',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC63", core=True)
    assert not gate.has_switch("HC63")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/executor/publication.rs"
    assert 'sumeragi_core_mutation = "HC63"' in source.read_text()


def test_committed_archive_capture_preserves_original_resource_owners():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC64"]
    assert rule.tests == (
        'sumeragi::executor::archive_tests::committed_archive_index_refusal_retains_original_release_and_exact_publication',
        'sumeragi::executor::archive_tests::committed_archive_cold_history_refusal_retains_original_pool_and_exact_publication',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC64", core=True)
    assert not gate.has_switch("HC64")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/executor.rs"
    assert 'sumeragi_core_mutation = "HC64"' in source.read_text()


def test_cold_preparation_preserves_original_execution_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC65"]
    assert rule.tests == (
        'sumeragi::executor::preparation::tests::cold_prepare_refusal_retains_original_finishing_owner_and_exact_release',
        'sumeragi::executor::preparation::tests::cold_prepare_validation_refusals_retain_original_storage_owners',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC65", core=True)
    assert not gate.has_switch("HC65")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/executor.rs"
    assert 'sumeragi_core_mutation = "HC65"' in source.read_text()


def test_original_cell_acquisition_preserves_nonblocking_physical_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC71"]
    assert rule.tests == (
        'sumeragi::executor::preparation::tests::cold_prepare_validation_refusals_retain_original_storage_owners',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC71", core=True)
    assert not gate.has_switch("HC71")
    source = gate.REPO / "crates/iroha_core/src/state/world_acquisition.rs"
    assert 'sumeragi_core_mutation = "HC71"' in source.read_text()


def test_physical_history_contention_ignores_logical_membership_release():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC73"]
    assert rule.tests == (
        'state::storage_transactions::history::tests::physical_history_busy_ignores_logical_cleanup_and_retries_actual_release',
        'sumeragi::executor::publication::tests::state_publication_lock_refusals_reach_scheduler_with_original_execution_and_release',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC73", core=True)
    assert not gate.has_switch("HC73")
    source = gate.REPO / "crates/iroha_core/src/state/storage_transactions.rs"
    assert 'sumeragi_core_mutation = "HC73"' in source.read_text()


def test_opaque_host_and_replay_preserve_signed_staking_authority():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC66"]
    assert rule.tests == (
        'executor::opaque_monetary_tests::raw_ivm_staking_trigger_requires_signed_monetary_plan',
        'executor::opaque_monetary_tests::supplied_proved_staking_effects_require_signed_monetary_plan',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC66", core=True)
    assert not gate.has_switch("HC66")
    source = gate.REPO / "crates/iroha_core/src/deferred_authority.rs"
    assert 'sumeragi_core_mutation = "HC66"' in source.read_text()


def test_live_multisig_proposals_preserve_original_decoder_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC67"]
    assert rule.tests == (
        'smartcontracts::isi::multisig::tests::proposal_attempt::live_multisig_proposal_decode_refusal_retries_original_signed_xor_claim',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC67", core=True)
    assert not gate.has_switch("HC67")
    source = gate.REPO / "crates/iroha_core/src/smartcontracts/isi/multisig.rs"
    assert 'sumeragi_core_mutation = "HC67"' in source.read_text()


def test_retained_multisig_proposals_bind_exact_approved_body_and_physical_row():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC68"]
    assert rule.tests == (
        'smartcontracts::isi::multisig::tests::proposal_attempt::live_multisig_proposal_body_binding_rolls_back_original_signed_xor_claim',
        'smartcontracts::isi::multisig::tests::proposal_attempt::proposal_migration_validates_original_physical_key_and_body_before_writes',
        'queue::router::tests::persisted_multisig_body_binding_and_local_refusal_reach_signed_queue_admission',
        'state::deserialize::decode_tests::restored_multisig_proposals_require_exact_body_and_preserve_local_read_refusal',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC68", core=True)
    assert not gate.has_switch("HC68")
    source = gate.REPO / "crates/iroha_core/src/smartcontracts/isi/multisig.rs"
    assert 'sumeragi_core_mutation = "HC68"' in source.read_text()


def test_multisig_cancellation_and_expiry_keep_original_custom_decode_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC69"]
    assert rule.tests == (
        'smartcontracts::isi::multisig::tests::proposal_attempt::cancel_wrapper_decode_refusal_rolls_back_and_retries_original_signed_approval',
        'smartcontracts::isi::multisig::tests::proposal_attempt::expiry_child_decode_refusal_rolls_back_and_retries_original_signed_approval',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC69", core=True)
    assert not gate.has_switch("HC69")
    source = gate.REPO / "crates/iroha_core/src/smartcontracts/isi/multisig.rs"
    assert 'sumeragi_core_mutation = "HC69"' in source.read_text()


def test_multisig_queue_traversal_preserves_native_depth_bound():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC70"]
    assert rule.tests == (
        'queue::router::tests::persisted_multisig_chain_is_checked_in_linear_expansions',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC70", core=True)
    assert not gate.has_switch("HC70")
    source = gate.REPO / "crates/iroha_core/src/queue/router.rs"
    assert 'sumeragi_core_mutation = "HC70"' in source.read_text()


def test_prepared_certificate_preserves_original_state_reader_release():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC72"]
    assert rule.tests == (
        'sumeragi::executor::validation_refusal_tests::prepared_certificate_busy_retries_same_execution_after_original_reader_release',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC72", core=True)
    assert not gate.has_switch("HC72")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/executor_attestation.rs"
    assert 'sumeragi_core_mutation = "HC72"' in source.read_text()


def test_da_cold_refunds_retain_original_writer_custody():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC75"]
    assert rule.tests == (
        'state::da_hydration::release_tests::original_cold_da_refunds_follow_all_rebuild_and_rewind_writers',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC75", core=True)
    assert not gate.has_switch("HC75")
    source = gate.REPO / "crates/iroha_core/src/state/da_hydration.rs"
    assert 'sumeragi_core_mutation = "HC75"' in source.read_text()


def test_committed_head_retries_require_the_original_source_release():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC74"]
    assert rule.tests == (
        'sumeragi::driver::exec::source_retry_tests::committed_head_waits_for_original_release_across_prepare_append_and_commit',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC74", core=True)
    assert not gate.has_switch("HC74")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/driver/exec.rs"
    assert 'sumeragi_core_mutation = "HC74"' in source.read_text()



def test_all_execution_producer_retries_require_original_physical_release():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC76"]
    assert rule.tests == (
        'sumeragi::driver::exec::producer_retry_tests::every_execution_producer_retains_original_source_until_actual_release',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC76", core=True)
    assert not gate.has_switch("HC76")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/driver/exec.rs"
    assert 'sumeragi_core_mutation = "HC76"' in source.read_text()



def test_cancelled_execution_producers_cannot_resume_when_context_returns():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC78"]
    assert rule.tests == (
        'sumeragi::driver::exec::producer_retry_tests::cancellation_stays_final_when_the_identical_context_and_request_return',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC78", core=True)
    assert not gate.has_switch("HC78")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/driver/exec.rs"
    assert 'sumeragi_core_mutation = "HC78"' in source.read_text()


def test_pending_ingress_admission_requires_its_exact_original_capacity_source():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC77"]
    assert rule.tests == (
        'sumeragi::driver::witness_admission_tests::pending_capacity_requires_original_release_despite_foreign_wake_and_huge_clock',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC77", core=True)
    assert not gate.has_switch("HC77")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/driver/mod.rs"
    assert 'sumeragi_core_mutation = "HC77"' in source.read_text()



def test_admitted_ingress_refunds_follow_both_original_mutexes():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC80"]
    assert rule.tests == (
        'sumeragi::driver::witness_admission_tests::admitted_ingress_eviction_refunds_only_after_pending_and_ingress_mutexes_release',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC80", core=True)
    assert not gate.has_switch("HC80")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/driver/mod.rs"
    assert 'sumeragi_core_mutation = "HC80"' in source.read_text()


def test_completed_replay_retains_original_encoding_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC79"]
    assert rule.tests == (
        'sumeragi::executor::replay::tests::completed_replay_retains_exact_receipt_through_original_pool_scratch_refusal',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC79", core=True)
    assert not gate.has_switch("HC79")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/executor/replay.rs"
    assert 'sumeragi_core_mutation = "HC79"' in source.read_text()


def test_native_beacon_startup_retains_original_readiness_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC81"]
    assert rule.tests == (
        "sumeragi::executor::publication_tests::beacon_startup_retains_original_capacity_through_worker_channel_and_node",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC81", core=True)
    assert not gate.has_switch("HC81")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/executor_control.rs"
    assert 'sumeragi_core_mutation = "HC81"' in source.read_text()


def test_scheduler_retirement_cancels_original_waiters_before_refunds():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC83"]
    assert rule.tests == (
        "sumeragi::driver::exec::producer_retry_tests::cancelling_and_dropping_scheduler_unlinks_all_waiters_before_original_refunds",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC83", core=True)
    assert not gate.has_switch("HC83")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/driver/exec.rs"
    assert 'sumeragi_core_mutation = "HC83"' in source.read_text()


def test_world_root_verification_retains_original_storage_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC82"]
    assert rule.tests == (
        'sumeragi::test_chain::tests::world_state_tests::world_root_verification_preserves_original_writer_refusal_and_exact_retry',
        'sumeragi::test_chain::tests::world_state_tests::world_root_verification_preserves_original_capacity_refusal_and_exact_retry',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC82", core=True)
    assert not gate.has_switch("HC82")
    source = gate.REPO / "crates/iroha_core/src/state/world_state_accumulator.rs"
    assert 'sumeragi_core_mutation = "HC82"' in source.read_text()


def test_startup_history_retains_original_cold_kura_read_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC84"]
    assert rule.tests == (
        "kura::tests::startup_history_retains_original_cold_kura_refusal_and_exact_retry",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC84", core=True)
    assert not gate.has_switch("HC84")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/driver/mod.rs"
    assert 'sumeragi_core_mutation = "HC84"' in source.read_text()
