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


def selected_cargo_results(monkeypatch, code, output, names=("tests::named",), seconds=1.0):
    """Supply a completed executable inventory and a separate runtime response."""
    def cargo_test(*args):
        if "--list" in args[3]:
            listing = "".join(name + ": test\n" for name in names)
            return 0, listing + f"\n{len(names)} tests, 0 benchmarks\n", 0.0
        return code, output, seconds
    monkeypatch.setattr(gate, "cargo_test", cargo_test)


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
        (0, "running 1 test\ntest tests::named ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n", "pass"),
        (101, "running 1 test\ntest tests::named ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n", "fail"),
        (None, "test tests::named ... ", "timeout"),
    ],
)
def test_only_executed_named_test_results_establish_a_mutation_kill(
    monkeypatch, tmp_path, code, output, expected
):
    selected_cargo_results(monkeypatch, code, output)
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
    selected_cargo_results(monkeypatch, 101, "test tests::present ... FAILED\n",
                           names=("tests::present", "tests::missing"))
    step = gate.run_step(
        SimpleNamespace(), tmp_path, "MS1", ["present", "missing"], None, 5,
        tmp_path / "named.log",
    )
    assert step.status == "missing-test"
    assert step.failed == ["tests::present"]


@pytest.mark.parametrize("terminals,summary", [
    ("test tests::named ... FAILED\n", "0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out;"),
    ("test tests::named ... FAILED\ntest tests::named ... FAILED\n", "0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out;"),
    ("test tests::named ... FAILED\ntest tests::named_extra ... ok\n", "0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out;"),
    ("test tests::named ... FAILED\ntest tests::named_extra ... ignored, deferred control\n", "0 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out;"),
    ("test tests::named ... FAILED\ntest tests::unexpected ... ok\n", "1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out;"),
])
def test_substring_selection_cannot_hide_missing_duplicate_ignored_or_wrong_results(
    monkeypatch, tmp_path, terminals, summary
):
    selected_cargo_results(monkeypatch, 101, terminals + "test result: FAILED. " + summary + "\n",
                           names=("tests::named", "tests::named_extra"))
    step = gate.run_step(SimpleNamespace(), tmp_path, "MS1", ["named"], None, 5,
                         tmp_path / "named.log")
    assert step.status == "execution-error"
    assert step.selected == ["tests::named", "tests::named_extra"]


@pytest.mark.parametrize("code,verdict,passed,failed", [(0, "ok", 2, 0), (101, "FAILED", 1, 1)])
def test_every_actual_substring_match_is_accounted_before_passing_or_killing(
    monkeypatch, tmp_path, code, verdict, passed, failed
):
    output = (f"running 2 tests\ntest tests::named ... {verdict}\ntest tests::named_extra ... ok\n"
              f"test result: {verdict}. {passed} passed; {failed} failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s\n")
    selected_cargo_results(monkeypatch, code, output,
                           names=("tests::named", "tests::named_extra"))
    step = gate.run_step(SimpleNamespace(), tmp_path, "MS1", ["named"], None, 5,
                         tmp_path / "named.log")
    assert step.status == ("fail" if failed else "pass")
    assert step.ran == step.selected


@pytest.mark.parametrize("listing", [
    "tests::named: test\n",  # Incomplete discovery.
    "tests::named: test\n\n2 tests, 0 benchmarks\n",  # Missing name.
    "tests::named: test\ntests::named: test\n\n2 tests, 0 benchmarks\n",  # Duplicate.
])
def test_incomplete_executable_discovery_cannot_run_or_kill_a_mutant(monkeypatch, tmp_path, listing):
    def cargo_test(*args):
        assert "--list" in args[3], "invalid discovery must stop before execution"
        return 0, listing, 0.0
    monkeypatch.setattr(gate, "cargo_test", cargo_test)
    step = gate.run_step(SimpleNamespace(), tmp_path, "MS1", ["named"], None, 5,
                         tmp_path / "named.log")
    assert step.status == "execution-error"


def test_discovery_and_execution_share_the_original_step_deadline(monkeypatch, tmp_path):
    moments = iter((100.0, 101.0, 102.0))
    monkeypatch.setattr(gate.time, "monotonic", lambda: next(moments))
    calls = []
    def cargo_test(*args):
        calls.append((args[3], args[5]))
        if "--list" in args[3]:
            return 0, "tests::named: test\n\n1 test, 0 benchmarks\n", 1.0
        return 0, ("running 1 test\ntest tests::named ... ok\ntest result: ok. 1 passed; 0 failed; "
                   "0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"), 1.0
    monkeypatch.setattr(gate, "cargo_test", cargo_test)
    step = gate.run_step(SimpleNamespace(), tmp_path, None, ["named"], None, 5,
                         tmp_path / "named.log")
    assert step.status == "pass"
    assert [timeout for _, timeout in calls] == [5, 4]
    assert "--test-threads=1" in calls[1][0]


def test_discovery_timeout_never_replenishes_or_disables_the_execution_deadline(monkeypatch, tmp_path):
    def cargo_test(*args):
        assert "--list" in args[3], "the exhausted deadline cannot launch a test"
        return 0, "tests::named: test\n\n1 test, 0 benchmarks\n", 5.0
    monkeypatch.setattr(gate, "cargo_test", cargo_test)
    step = gate.run_step(SimpleNamespace(), tmp_path, None, ["named"], None, 5,
                         tmp_path / "named.log")
    assert step.status == "timeout"


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
    assert captured["command"][:10] == ["cargo", "test", "--locked", "-p", crate, "--release", "--features", features, "--lib", "--"]
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
    functions.update(re.findall(r"\bstate_test!\s*\{\s*(?:sync|result|large_stack|consensus_stack)\s+(\w+)\b", text))
    ids = [mutation.id for mutation in gate.CORE_MUTATIONS]
    assert len(ids) == len(set(ids))
    for mutation in gate.CORE_MUTATIONS:
        assert gate.has_switch(mutation.id, core=True)
        assert not gate.has_switch(mutation.id)
        assert mutation.tests and not mutation.scenarios
        assert all(name.rsplit("::", 1)[-1] in functions for name in mutation.tests)


def test_mutation_ids_select_one_rule_across_all_implementation_owners():
    mutations = [*gate.MUTATIONS, *gate.CORE_MUTATIONS, *gate.DAEMON_MUTATIONS]
    assert len(gate.index_mutations(mutations)) == len(mutations)
    core_and_daemon = gate.index_mutations([*gate.CORE_MUTATIONS, *gate.DAEMON_MUTATIONS])
    rows = re.findall(
        r"^\| (HC\d+) \| (.+)$", (ROOT / "specs/sumeragi.md").read_text(), re.MULTILINE
    )
    identifiers = [identifier for identifier, _ in rows]
    assert len(identifiers) == len(set(identifiers))
    assert set(identifiers) == set(core_and_daemon)
    for identifier, row in rows:
        for name in core_and_daemon[identifier].tests:
            assert name.rsplit("::", 1)[-1] in row, (identifier, name)


def test_core_custody_mutations_have_distinct_source_owners():
    expected = {
        "HC120": {"sumeragi/evidence_history.rs"},
        "HC121": {"state/output_capacity.rs"},
        "HC122": {"sumeragi/lanes/registry.rs"},
        "HC123": {"sumeragi/lanes/store.rs"},
        "HC124": {"sumeragi/lanes/registry.rs"},
        "HC125": {"sumeragi/lanes/registry.rs"},
        "HC126": {"sumeragi/crypto.rs"},
        "HC127": {"snapshot.rs"},
    }
    registered = gate.index_mutations(gate.CORE_MUTATIONS)
    source = gate.REPO / "crates/iroha_core/src"
    owners = {identifier: set() for identifier in expected}
    for path in source.rglob("*.rs"):
        for identifier in re.findall(r'sumeragi_core_mutation\s*=\s*"([^"]+)"', path.read_text()):
            if identifier in owners:
                owners[identifier].add(path.relative_to(source).as_posix())
    assert owners == expected
    for identifier in expected:
        assert registered[identifier].tests
        assert not registered[identifier].scenarios
        assert not gate.has_switch(identifier)
        assert not gate.has_switch(identifier, daemon=True)


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
    assert captured[0][5:7] == ["--profile", profile]
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


def test_hc12_selects_only_the_original_beacon_complete_prepaid_owner():
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
        'validation_fee::tests::registry_refusal_tests::signed_fee_runtime_read_does_not_turn_scope_refusal_into_a_nonmatching_origin',
        'validation_fee::tests::registry_refusal_tests::original_retained_fee_registry_does_not_publish_local_decode_refusal_as_malformed',
        'smartcontracts::isi::world::isi::tests::signed_payout_scope_refusal_cannot_publish_a_parliament_terminal_outcome',
    )
    assert not rule.scenarios
    assert gate.has_switch("HC30", core=True)
    assert not gate.has_switch("HC30")


def test_core_credit_reader_gate_requires_alias_and_owner_original_source_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC32"]
    assert rule.tests == (
        "validation_fee_rewards::tests::original_fee_credit_alias_decode_refusal_preserves_balance_and_retries",
        "validation_fee_rewards::tests::original_fee_credit_owner_decode_refusal_preserves_exact_binding_and_retries",
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
    assert rule.tests == ('state::validator_committee::tests::refusal::original_incumbent_history_refusal_keeps_authority_and_same_source_retry', 'state::validator_committee::tests::refusal::original_credentials_authority_refusal_keeps_command_and_same_source_retry', 'state::validator_committee::tests::refusal::original_credentials_command_decode_refusal_has_no_publication_and_retries', 'state::validator_committee::tests::refusal::original_beacon_public_state_decode_refusal_defers_before_installation', 'state::validator_committee::tests::refusal::original_tle_public_state_decode_refusal_defers_before_installation', 'state::validator_committee::tests::refusal::original_staking_authority_refusal_keeps_exit_overlay_and_same_signed_retry')
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
    selected_cargo_results(monkeypatch, code, output, seconds=8)
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


def test_stake_index_quantities_have_a_distinct_original_pool_kill_control():
    rules = gate.index_mutations(gate.CORE_MUTATIONS)
    rule = rules["HC87"]
    assert rule.tests == (
        "smartcontracts::isi::staking::tests::stake_index_quantities_prepaid_and_borrowed_from_original_pool",
    )
    assert not rule.scenarios
    assert rule.id != rules["HC55"].id
    assert gate.has_switch("HC87", core=True)
    assert not gate.has_switch("HC87")


def test_replay_configuration_has_a_distinct_original_source_kill_control():
    rules = gate.index_mutations(gate.CORE_MUTATIONS)
    rule = rules["HC94"]
    assert rule.tests == (
        "sumeragi::executor::publication_tests::replay_completion_retirement_keeps_exact_source_and_original_pool_retry",
    )
    assert not rule.scenarios
    assert rule.id != rules["HC56"].id
    assert gate.has_switch("HC94", core=True)
    assert not gate.has_switch("HC94")


def test_native_amx_binding_has_a_distinct_original_paid_restart_kill_control():
    rules = gate.index_mutations(gate.CORE_MUTATIONS)
    rule = rules["HC95"]
    assert rule.tests == (
        "sumeragi::amx::native::tests::native_amx_paid_commit_survives_certified_restart_and_rejects_bypass",
    )
    assert not rule.scenarios
    assert rule.id != rules["HC54"].id
    assert gate.has_switch("HC95", core=True)
    assert not gate.has_switch("HC95")
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
    source = gate.REPO / "crates/iroha_core/src/sumeragi/executor.rs"
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


def test_shared_beacon_owner_requires_complete_current_external_binding():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC85"]
    assert rule.tests == (
        "beacon::session_owner::validated::tests::"
        "shared_authenticated_session_rechecks_every_current_external_binding",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC85", core=True)
    assert not gate.has_switch("HC85")
    source = gate.REPO / "crates/iroha_core/src/beacon/session_owner/validated.rs"
    assert 'sumeragi_core_mutation = "HC85"' in source.read_text()


def test_original_local_custody_invariant_requires_native_recovery():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC86"]
    assert rule.tests == (
        "sumeragi::executor::preparation::tests::"
        "original_local_custody_invariant_halts_worker_without_fee_result_or_quarantine",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC86", core=True)
    assert not gate.has_switch("HC86")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/executor/preparation.rs"
    assert 'sumeragi_core_mutation = "HC86"' in source.read_text()


def test_stake_index_and_signed_reward_controls_have_distinct_source_identities():
    indexed = gate.index_mutations(gate.CORE_MUTATIONS)
    assert indexed["HC55"].tests == (
        "validation_fee_rewards::tests::signed_fee_reward_claim_rejects_every_changed_binding_before_mutation",
    )
    assert indexed["HC87"].tests == (
        "smartcontracts::isi::staking::tests::stake_index_quantities_prepaid_and_borrowed_from_original_pool",
    )
    assert not indexed["HC87"].scenarios
    assert gate.has_switch("HC55", core=True)
    assert gate.has_switch("HC87", core=True)
    assert not gate.has_switch("HC87")
    stake = (gate.REPO / "crates/iroha_core/src/smartcontracts/isi/staking.rs").read_text()
    reward = (gate.REPO / "crates/iroha_core/src/validation_fee_rewards.rs").read_text()
    assert 'sumeragi_core_mutation = "HC87"' in stake
    assert 'sumeragi_core_mutation = "HC55"' not in stake
    assert 'sumeragi_core_mutation = "HC55"' in reward
    assert 'sumeragi_core_mutation = "HC87"' not in reward


def test_beacon_decoder_preserves_actual_local_decode_scope():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC88"]
    assert rule.tests == (
        "beacon::tests::session_decoder_preserves_actual_local_scope_without_invalidity_or_fabricated_pool",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC88", core=True)
    assert not gate.has_switch("HC88")
    source = gate.REPO / "crates/iroha_core/src/beacon.rs"
    assert 'sumeragi_core_mutation = "HC88"' in source.read_text()


def test_credential_decoder_preserves_captured_original_scope_and_allocator_cause():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC89"]
    assert rule.tests == (
        "beacon::credential::tests::credential_decoder_captures_original_scope_before_unwind_and_retries_unchanged_bytes",
        "beacon::credential::tests::credential_decoder_physical_refusal_keeps_exact_allocator_cause_and_retries",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC89", core=True)
    assert not gate.has_switch("HC89")
    source = gate.REPO / "crates/iroha_core/src/beacon/credential.rs"
    assert 'sumeragi_core_mutation = "HC89"' in source.read_text()


def test_native_journal_preserves_original_control_refusal():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC90"]
    assert rule.tests == (
        "sumeragi::native_journal::tests::native_cursor_preserves_original_pool_refusal_and_retries_identical_prefix",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC90", core=True)
    assert not gate.has_switch("HC90")
    source = gate.REPO / "crates/iroha_core/src/sumeragi/native_journal.rs"
    assert 'sumeragi_core_mutation = "HC90"' in source.read_text()


def test_live_dkg_owns_physical_outputs_before_durable_claim_and_randomness():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC91"]
    assert rule.tests == (
        "beacon::dkg_local_seat::ownership_tests::prepared_local_outputs_are_complete_before_randomness_at_four_and_thirty_one",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC91", core=True)
    assert not gate.has_switch("HC91")
    source = gate.REPO / "crates/iroha_core/src/beacon/session_owner/dkg/pending.rs"
    assert 'sumeragi_core_mutation = "HC91"' in source.read_text()


def test_beacon_credential_output_uses_only_its_pre_extraction_physical_backing():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC92"]
    assert rule.tests == (
        "beacon::credential::prepared_output::tests::prepared_credential_uses_exact_original_output_without_late_growth_at_four_and_thirty_one",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC92", core=True)
    assert not gate.has_switch("HC92")
    source = gate.REPO / "crates/iroha_core/src/beacon/credential/prepared_output.rs"
    assert 'sumeragi_core_mutation = "HC92"' in source.read_text()


def test_daemon_beacon_mutation_has_its_actual_owner_and_exact_regression():
    rule = gate.index_mutations(gate.DAEMON_MUTATIONS)["HC93"]
    assert rule.tests == (
        "runtime_provider_broker::protocol::platform::tests::beacon_operation_reuses_original_graph_across_ingress_dispatch_and_response",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC93", daemon=True)
    assert not gate.has_switch("HC93", core=True)
    assert not gate.has_switch("HC93")
    with pytest.raises(ValueError, match="exactly one"):
        gate.has_switch("HC93", daemon=True, core=True)


def test_daemon_mutation_table_cannot_select_a_dependency_rule(monkeypatch, capsys):
    monkeypatch.setattr(sys, "argv", ["sumeragi_mutation_gate.py", "--daemon", "--list"])
    assert gate.main() == 0
    listing = capsys.readouterr().out
    assert "HC93" in listing
    assert "HC92" not in listing
    assert "MS1" not in listing


@pytest.mark.parametrize("arguments", [
    ["--daemon", "--core"], ["--daemon", "--only", "HC92"],
    ["--core", "--only", "HC93"], ["--only", "HC93"],
    ["--daemon", "--core-profile", "test"],
])
def test_daemon_mutation_owner_and_profile_are_exact(monkeypatch, arguments):
    monkeypatch.setattr(sys, "argv", ["sumeragi_mutation_gate.py", *arguments])
    with pytest.raises(SystemExit) as error:
        gate.main()
    assert error.value.code == 2


@pytest.mark.parametrize("owner", ["daemon", "core", "protocol"])
@pytest.mark.parametrize("mutation", [None, "HC93"])
def test_daemon_mutation_environment_never_reaches_dependency_owners(
    monkeypatch, tmp_path, owner, mutation
):
    captured = {}
    class Process:
        returncode = 0
        def communicate(self, *, timeout):
            return "test result: ok. 1 passed; 0 failed;", None
    def popen(command, **options):
        captured.update(command=command, environment=options["env"])
        return Process()
    for name in ["SUMERAGI_MUTATION", "SUMERAGI_CORE_MUTATION", "SUMERAGI_DAEMON_MUTATION"]:
        monkeypatch.setenv(name, "inherited-foreign-rule")
    monkeypatch.setattr(gate.subprocess, "Popen", popen)
    args = SimpleNamespace(core=owner == "core", daemon=owner == "daemon")
    code, _, _ = gate.cargo_test(args, tmp_path, mutation, ["named"], None, 0, tmp_path / "log")
    assert code == 0
    package, features, selected_environment = gate.package_options(args)
    assert captured["command"][:9] == [
        "cargo", "test", "--locked", "-p", package, "--release", "--features", features, "--lib"
    ]
    if owner == "daemon":
        assert (package, features, selected_environment) == (
            "irohad_lib", "mutation-testing", "SUMERAGI_DAEMON_MUTATION"
        )
    for name in ["SUMERAGI_MUTATION", "SUMERAGI_CORE_MUTATION", "SUMERAGI_DAEMON_MUTATION"]:
        if mutation is not None and name == selected_environment:
            assert captured["environment"][name] == mutation
        else:
            assert name not in captured["environment"]


@pytest.mark.parametrize("status", ["execution-error", "missing-test", "timeout"])
def test_daemon_gate_requires_a_named_test_failure(monkeypatch, tmp_path, status):
    monkeypatch.setattr(gate, "has_switch", lambda _, *, daemon: daemon)
    monkeypatch.setattr(gate, "build", lambda *args: gate.Step(status="pass"))
    monkeypatch.setattr(gate, "run_step", lambda *args: gate.Step(status=status))
    args = SimpleNamespace(daemon=True, target_dir=tmp_path, timeout_test=0, fast=True)
    result = gate.evaluate(args, tmp_path, gate.DAEMON_MUTATIONS[0])
    assert result["verdict"] == "error"
    assert result["reason"] == f"named tests: {status}"


@pytest.mark.parametrize("test_build,mutation_feature,accepted", [
    (False, False, True), (True, False, True),
    (True, True, True), (False, True, False),
])
def test_daemon_guard_rejects_mutation_features_in_non_test_builds(
    tmp_path, test_build, mutation_feature, accepted
):
    guard = ROOT / "crates/irohad/src/mutation_guard.rs"
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


@pytest.fixture(scope="module")
def daemon_build_script(tmp_path_factory):
    executable = tmp_path_factory.mktemp("daemon-mutation-build") / "build-script"
    result = subprocess.run(
        ["rustc", "--edition=2024", str(ROOT / "crates/irohad/build.rs"),
         "-o", str(executable)], capture_output=True, text=True, check=False,
    )
    assert result.returncode == 0, result.stderr
    return executable


@pytest.mark.parametrize("feature,mutation,flags,accepted,emitted", [
    (False, "HC93", "", True, False),
    (False, None, '--cfg\x1fsumeragi_daemon_mutation="HC93"', False, False),
    (True, "HC93", "", True, True),
    (True, None, "", True, False),
    (True, "unknown_rule", "", False, False),
    (True, 'HC93"', "", False, False),
])
def test_daemon_build_script_has_no_shipping_fault_control(
    daemon_build_script, feature, mutation, flags, accepted, emitted
):
    environment = dict(os.environ)
    for key in ("CARGO_FEATURE_MUTATION_TESTING", "SUMERAGI_DAEMON_MUTATION"):
        environment.pop(key, None)
    environment["SUMERAGI_MUTATION"] = "foreign-protocol-rule"
    environment["SUMERAGI_CORE_MUTATION"] = "foreign-core-rule"
    environment["CARGO_ENCODED_RUSTFLAGS"] = flags
    if feature:
        environment["CARGO_FEATURE_MUTATION_TESTING"] = "1"
    if mutation is not None:
        environment["SUMERAGI_DAEMON_MUTATION"] = mutation
    result = subprocess.run(
        [str(daemon_build_script)], cwd=ROOT / "crates/irohad", env=environment,
        capture_output=True, text=True, check=False,
    )
    assert (result.returncode == 0) == accepted, result.stderr
    assert ('cargo:rustc-cfg=sumeragi_daemon_mutation="HC93"' in result.stdout) == emitted
    assert "cargo:rustc-cfg=sumeragi_mutation=" not in result.stdout
    assert "cargo:rustc-cfg=sumeragi_core_mutation=" not in result.stdout
    if not feature and mutation:
        assert "ignored" in result.stdout


def capture_command(monkeypatch,tmp_path):
    captured={}
    class Child:
        returncode=0
        def communicate(self,*,timeout=None): return '',None
    def popen(command,**kwargs):
        captured.update(command=command,seed_base=kwargs['env'].get('SUMERAGI_SIM_SEED_BASE'))
        return Child()
    monkeypatch.setattr(gate.subprocess,'Popen',popen)
    gate.cargo_test(SimpleNamespace(),tmp_path,None,[],None,900,tmp_path/'cargo.log',no_run=True)
    return captured

def test_actual_mutation_command_cannot_resolve_an_unlocked_dependency_graph(monkeypatch,tmp_path):
    assert '--locked' in capture_command(monkeypatch,tmp_path)['command']

def test_actual_mutation_command_rejects_inherited_simulator_seed_base(monkeypatch,tmp_path):
    monkeypatch.setenv('SUMERAGI_SIM_SEED_BASE','123456789')
    assert capture_command(monkeypatch,tmp_path)['seed_base'] is None

@pytest.mark.parametrize('scenarios,expected',[ ((),[900]), (('F1',),[900,3600]) ])
def test_actual_baseline_keeps_original_named_and_scenario_deadlines(monkeypatch,tmp_path,scenarios,expected):
    if scenarios: scenarios=(next(iter(gate.SCENARIOS)),)
    calls=[]
    monkeypatch.setattr(gate,'build',lambda *args:gate.Step(status='pass'))
    def step(args,target,mutation,filters,seeds,timeout,log):
        calls.append(timeout);return gate.Step(status='pass')
    monkeypatch.setattr(gate,'run_step',step)
    args=SimpleNamespace(target_dir=tmp_path,timeout_test=900,timeout_scenario=3600,seeds=200,fast=False)
    result=gate.evaluate_baseline(args,tmp_path,[SimpleNamespace(tests=('tests::named',),scenarios=scenarios)])
    assert result['verdict']=='pass'
    assert calls==expected,'each original invocation must retain its configured deadline'

def setup_main(monkeypatch,tmp_path,extra=()):
    monkeypatch.setattr(sys,'argv',['sumeragi_mutation_gate.py','--only','MS1','--fast','--jobs','1','--target-dir',str(tmp_path),*extra])
    captured={}
    def baseline(args,*rest):
        captured['build_cap']=args.timeout_build
        return {'id':'baseline','verdict':'pass'}
    monkeypatch.setattr(gate,'evaluate_baseline',baseline)
    monkeypatch.setattr(gate,'evaluate',lambda args,target,mu:{'id':mu.id,'verdict':'killed_by_test','named':{'failed':['tests::named']}})
    return captured

def test_actual_mutation_default_build_deadline_is_twenty_minutes(monkeypatch,tmp_path):
    captured=setup_main(monkeypatch,tmp_path)
    assert gate.main()==0
    assert captured['build_cap']==1200

def test_actual_strict_gate_cannot_pass_without_an_unmutated_baseline(monkeypatch,tmp_path):
    setup_main(monkeypatch,tmp_path,('--strict','--skip-baseline'))
    with pytest.raises(SystemExit) as error: gate.main()
    assert error.value.code==2


@pytest.mark.parametrize("timeout", [900, 1200, 3600])
@pytest.mark.parametrize("completed_code", [0, 101])
def test_mutation_command_retains_cap_failure_after_late_natural_completion(
    monkeypatch, tmp_path, timeout, completed_code
):
    output = "test tests::named ... " + ("ok" if completed_code == 0 else "FAILED") + "\n"
    class Process:
        returncode = completed_code
        def communicate(self, *, timeout=None):
            return output, None
    moments = iter((100.0, 100.0 + timeout + 1.0))
    monkeypatch.setattr(gate.time, "monotonic", lambda: next(moments))
    monkeypatch.setattr(gate.subprocess, "Popen", lambda *_args, **_kwargs: Process())
    code, retained, elapsed = gate.cargo_test(
        SimpleNamespace(), tmp_path, None, ["named"], None, timeout, tmp_path / "late.log"
    )
    assert elapsed == timeout + 1.0
    assert retained == output
    assert code is None, "a late natural completion cannot pass or kill a mutant"


def test_mutation_command_keeps_explicit_unbounded_diagnostic_wait(
    monkeypatch, tmp_path
):
    class Process:
        returncode = 0
        def communicate(self, *, timeout=None):
            assert timeout is None
            return "retained original output", None
    moments = iter((100.0, 10000.0))
    monkeypatch.setattr(gate.time, "monotonic", lambda: next(moments))
    monkeypatch.setattr(gate.subprocess, "Popen", lambda *_args, **_kwargs: Process())
    code, retained, elapsed = gate.cargo_test(
        SimpleNamespace(), tmp_path, None, [], None, 0, tmp_path / "unbounded.log"
    )
    assert (code, retained, elapsed) == (0, "retained original output", 9900.0)


def test_nightly_runs_every_actual_daemon_mutation_and_retains_its_report():
    workflow = (ROOT / ".github/workflows/nightly_sumeragi.yml").read_text()
    match = re.search(r"(?ms)^  daemon_mutation_gate:\n(.*?)(?=^  [a-z_]+:|\Z)", workflow)
    assert match is not None, "daemon rules need their own maintained nightly owner"
    job = match.group(1)
    command = "python3 scripts/sumeragi_mutation_gate.py --daemon --jobs 1 --strict --fast"
    assert f"run: {command}\n" in job
    assert "--only" not in job, "nightly qualification must cover the complete owner table"
    assert "if: always()" in job
    assert "target/sumeragi-daemon-mutants/report.json" in job
    assert "target/sumeragi-daemon-mutants/logs" in job
    assert "sumeragi-daemon-mutation-gate-${{ github.run_id }}" in job


def test_nightly_core_mutations_keep_dependency_instrumentation_and_full_inventory():
    workflow = (ROOT / ".github/workflows/nightly_sumeragi.yml").read_text()
    match = re.search(r"(?ms)^  core_mutation_gate:\n(.*?)(?=^  [a-z_]+:|\Z)", workflow)
    assert match is not None
    job = match.group(1)
    command = (
        "python3 scripts/sumeragi_mutation_gate.py --core --core-profile test "
        "--jobs 1 --strict --fast"
    )
    assert f"run: {command}\n" in job
    assert "--only" not in job
    assert not any(option in job for option in (
        "--skip-baseline", "--timeout-build", "--timeout-test", "--timeout-scenario",
    ))
    assert "if: always()" in job
    assert "target/sumeragi-core-mutants/report.json" in job
    assert "target/sumeragi-core-mutants/logs" in job


SUMERAGI_CI_JOBS = (
    ("nightly_sumeragi.yml", "simulator"),
    ("nightly_sumeragi.yml", "mutation_gate"),
    ("nightly_sumeragi.yml", "core_mutation_gate"),
    ("nightly_sumeragi.yml", "daemon_mutation_gate"),
    ("pr.yml", "sumeragi"),
)


def require_pinned_sumeragi_toolchain(job):
    """Check compiler selection before restoring or executing native artifacts."""
    channel = re.search(
        r'^channel\s*=\s*"([^"]+)"$',
        (ROOT / "rust-toolchain.toml").read_text(), re.MULTILINE,
    )
    assert channel is not None
    action = (
        "      - uses: actions-rust-lang/setup-rust-toolchain@"
        "166cdcfd11aee3cb47222f9ddb555ce30ddb9659\n"
        "        with:\n"
        '          cache: "false"\n'
        f"          toolchain: {channel.group(1)}\n"
    )
    assert action in job, "Sumeragi must install the repository-pinned Rust toolchain"
    setup = job.index(action)
    assert setup < job.index("      - uses: Swatinem/rust-cache@")
    assert setup < job.index("        run:")


@pytest.mark.parametrize("workflow_name,job_name", SUMERAGI_CI_JOBS)
def test_sumeragi_ci_installs_current_compiler_before_cache_and_execution(
    workflow_name, job_name
):
    workflow = (ROOT / ".github/workflows" / workflow_name).read_text()
    match = re.search(rf"(?ms)^  {job_name}:\n(.*?)(?=^  [a-z_]+:|\Z)", workflow)
    assert match is not None
    require_pinned_sumeragi_toolchain(match.group(1))


@pytest.mark.parametrize("change", ("missing", "stale", "unreviewed", "late"))
def test_sumeragi_ci_rejects_missing_stale_or_late_compiler_selection(change):
    workflow = (ROOT / ".github/workflows/nightly_sumeragi.yml").read_text()
    match = re.search(r"(?ms)^  simulator:\n(.*?)(?=^  [a-z_]+:|\Z)", workflow)
    assert match is not None
    job = match.group(1)
    action = re.search(
        r"(?m)^      - uses: actions-rust-lang/setup-rust-toolchain@[^\n]+\n"
        r"        with:\n          cache: [^\n]+\n          toolchain: [^\n]+\n", job,
    )
    assert action is not None
    if change == "missing":
        job = job.replace(action.group(0), "")
    elif change == "stale":
        job = job.replace(action.group(0), re.sub(
            r"toolchain: [^\n]+", "toolchain: retired-compiler", action.group(0),
        ))
    elif change == "unreviewed":
        job = job.replace("166cdcfd11aee3cb47222f9ddb555ce30ddb9659", "v1")
    else:
        job = job.replace(action.group(0), "") + action.group(0)
    with pytest.raises(AssertionError):
        require_pinned_sumeragi_toolchain(job)


def test_committee_boundary_mutations_use_their_exact_production_source_owners():
    registered = gate.index_mutations(gate.CORE_MUTATIONS)
    expected = {
        "HC100": "genuine_candidate_pools_choose_largest_equal_vote_committee",
        "HC101": "prepared_boundary_readiness_requires_every_frozen_seat_custody",
        "HC102": "prepared_boundary_readiness_requires_every_frozen_seat_custody",
        "HC103": "frozen_boundary_refusal_returns_original_pool_and_does_not_need_fresh_incumbent_keys",
    }
    for identifier, test in expected.items():
        rule = registered[identifier]
        assert rule.tests == (f"sumeragi::epoch_election::tests::{test}",)
        assert not rule.scenarios
        assert gate.has_switch(identifier, core=True)
        assert not gate.has_switch(identifier)
        assert not gate.has_switch(identifier, daemon=True)
    plan = (gate.REPO / "crates/iroha_core/src/sumeragi/epoch_election/plan.rs").read_text()
    assert "let ready = prepared_committee_ready(&source, transition);" in plan
    assert 'cfg!(all(test, sumeragi_core_mutation = "HC102"))' in plan
    assert '#[cfg(all(test, sumeragi_core_mutation = "HC101"))]' in plan
    assert '#[cfg(all(test, sumeragi_core_mutation = "HC103"))]' in plan


def test_native_lane_finalizer_gate_requires_original_validator_return_and_publication():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC133"]
    assert rule.tests == (
        "sumeragi::executor::validation_refusal_tests::original_lane_finalizer_refusal_returns_same_graph_before_seal_and_publishes_after_retry",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC133", core=True)
    assert not gate.has_switch("HC133")


def test_native_witness_handoff_gate_requires_source_guard_and_one_shot_recovery():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC134"]
    assert rule.tests == (
        "sumeragi::executor::validation_refusal_tests::validated_witness_guard_failure_requires_recovery_without_reexecuting_original_source",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC134", core=True)
    assert not gate.has_switch("HC134")
    assert not gate.has_switch("HC134", daemon=True)


def test_borrowed_amx_clone_gate_requires_actual_paid_persisted_source():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC135"]
    assert rule.tests == (
        "sumeragi::amx::native::tests::paid_borrowed_custody::native_amx_persisted_paid_borrowed_prepared_proof_clone_retains_original_graph_and_lifetime",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC135", core=True)
    assert not gate.has_switch("HC135")
    assert not gate.has_switch("HC135", daemon=True)


def test_native_lane_refusal_gate_requires_both_actual_original_pool_boundaries():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC136"]
    assert rule.tests == (
        "sumeragi::lanes::custody::tests::original_signer_pinning_refuses_then_retries_the_same_pool_and_stake_cut",
        "sumeragi::lanes::step::sample_owner_tests::sample_finalizer_refusal_preserves_exact_source_and_retry_funds_only_suffix",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC136", core=True)
    assert not gate.has_switch("HC136")
    assert not gate.has_switch("HC136", daemon=True)


def test_queue_resident_gate_requires_actual_last_owner_pressure_and_retry():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC137"]
    assert rule.tests == (
        "queue::tests::resident_owner_tests::removed_pending_owner_retains_original_resident_credit_until_last_reader",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC137", core=True)
    assert not gate.has_switch("HC137")
    assert not gate.has_switch("HC137", daemon=True)


def test_queue_cold_fence_gate_requires_actual_thread_and_original_refund():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC138"]
    assert rule.tests == (
        "queue::tests::resident_owner_tests::cold_queue_retirement_holds_original_fence_until_first_admission_can_publish",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC138", core=True)
    assert not gate.has_switch("HC138")
    assert not gate.has_switch("HC138", daemon=True)


def test_pending_payload_lease_mutations_bind_distinct_actual_original_boundaries():
    expected = {
        "HC140": "queue::payload_leases::tests::pending_payload_lease_retires_on_actual_certified_state_publication",
        "HC141": "queue::payload_leases::tests::pending_payload_selection_cannot_adopt_clear_and_readmission_during_selection",
        "HC142": "queue::payload_leases::tests::pending_payload_lease_uses_original_backing_and_retires_on_expiry_withdrawal_or_foreign_queue",
        "HC143": "queue::payload_leases::tests::pending_payload_lease_uses_original_backing_and_retires_on_expiry_withdrawal_or_foreign_queue",
        "HC144": "queue::payload_leases::tests::pending_payload_lease_preserves_original_capacity_refusal_and_refuses_generation_wrap",
        "HC145": "queue::payload_leases::tests::pending_payload_lease_preserves_original_capacity_refusal_and_refuses_generation_wrap",
    }
    registered = gate.index_mutations(gate.CORE_MUTATIONS)
    source = gate.REPO / "crates/iroha_core/src/queue/payload_leases.rs"
    text = source.read_text()
    for identifier, name in expected.items():
        rule = registered[identifier]
        assert rule.tests == (name,)
        assert not rule.scenarios
        assert gate.has_switch(identifier, core=True)
        assert not gate.has_switch(identifier)
        assert not gate.has_switch(identifier, daemon=True)
        assert 'fn ' + name.rsplit("::", 1)[-1] + '(' in text
        assert 'all(test, sumeragi_core_mutation = "' + identifier + '")' in text
        owners = {path.relative_to(gate.REPO / "crates/iroha_core/src").as_posix()
                  for path in (gate.REPO / "crates/iroha_core/src").rglob("*.rs")
                  if 'sumeragi_core_mutation = "' + identifier + '"' in path.read_text()}
        assert owners == {"queue/payload_leases.rs"}


def test_signed_root_gate_requires_actual_bounded_worker_and_original_parent_controls():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC139"]
    assert rule.tests == (
        "sumeragi::executor::validation_refusal_tests::prepared_certificate_uses_bounded_signed_root_without_rewalking_execution_history",
        "sumeragi::executor::validation_refusal_tests::successor_context_uses_original_parent_and_bounded_signed_root_without_history_rewalk",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC139", core=True)
    assert not gate.has_switch("HC139")
    assert not gate.has_switch("HC139", daemon=True)


@pytest.mark.parametrize("code,status,passed,failed", [(0, "ok", 1, 0), (101, "FAILED", 0, 1)])
@pytest.mark.parametrize("shape", ["missing_running", "duplicate_running", "wrong_running", "missing_duration", "bad_duration", "trailing_summary", "duplicate_summary", "malformed_extra_summary", "summary_before_terminal", "header_after_terminal"])
def test_incomplete_or_conflicting_full_completion_cannot_pass_or_kill(
    monkeypatch, tmp_path, code, status, passed, failed, shape
):
    header = "running 1 test\n"
    summary = (f"test result: {status}. {passed} passed; {failed} failed; 0 ignored; "
               "0 measured; 0 filtered out; finished in 0.01s\n")
    if shape == "missing_running":
        header = ""
    elif shape == "duplicate_running":
        header *= 2
    elif shape == "wrong_running":
        header = "running 2 tests\n"
    elif shape == "missing_duration":
        summary = summary.replace(" finished in 0.01s", "")
    elif shape == "bad_duration":
        summary = summary.replace("0.01s", "unobserved")
    elif shape == "trailing_summary":
        summary = summary.rstrip() + " unexpected\n"
    elif shape == "duplicate_summary":
        summary *= 2
    elif shape == "malformed_extra_summary":
        summary += "test result: incomplete\n"
    terminal = f"test tests::named ... {status}\n"
    output = header + terminal + summary
    if shape == "summary_before_terminal":
        output = header + summary + terminal
    elif shape == "header_after_terminal":
        output = terminal + header + summary
    selected_cargo_results(monkeypatch, code, output)
    result = gate.run_step(SimpleNamespace(), tmp_path, "MS1", ["named"], None, 5,
                           tmp_path / "named.log")
    assert result.status == "execution-error"


def synthetic_scheduling_args(tmp_path, *, fast=False):
    """Use isolated Python scheduling fixtures, never positive native qualification."""
    return SimpleNamespace(target_dir=tmp_path, timeout_build=1200, timeout_test=900,
                           timeout_scenario=3600, seeds=200, fast=fast)


def test_baseline_keeps_distinct_ordered_named_tuples_and_scenario_union(monkeypatch, tmp_path):
    scenarios = list(gate.SCENARIOS)[:2]
    mutations = [gate.m('SYNTHETIC_A', 'fixture', ['tests::b', 'tests::a'], scenarios),
                 gate.m('SYNTHETIC_B', 'fixture', ['tests::c'], [scenarios[0]]),
                 gate.m('SYNTHETIC_C', 'fixture', ['tests::b', 'tests::a'], [scenarios[1]])]
    builds, calls = [], []
    monkeypatch.setattr(gate, 'build', lambda *args: builds.append(args) or gate.Step(status='pass'))
    def run_step(args, target, mutation, filters, seeds, timeout, log):
        calls.append((tuple(filters), seeds, timeout, log))
        return gate.Step(status='pass', selected=list(filters), ran=list(filters), log=str(log))
    monkeypatch.setattr(gate, 'run_step', run_step)
    result = gate.evaluate_baseline(synthetic_scheduling_args(tmp_path), tmp_path, mutations)
    assert result['verdict'] == 'pass'
    assert len(builds) == 1 and builds[0][2] is None
    expected_tuples = sorted({mu.tests for mu in mutations})
    expected_scenarios = sorted({gate.SCENARIOS[s] for mu in mutations for s in mu.scenarios})
    assert [filters for filters, seeds, _, _ in calls if seeds is None] == expected_tuples
    assert [filters for filters, seeds, _, _ in calls if seeds == 200] == [(s,) for s in expected_scenarios]
    assert all(cap == (900 if seeds is None else 3600) for _, seeds, cap, _ in calls)
    assert len({log for _, _, _, log in calls}) == len(calls)
    assert result['named']['ran'] == sorted({t for mu in mutations for t in mu.tests})
    assert result['scenario']['ran'] == expected_scenarios
    assert result['named']['steps'][0]['filters'] == list(expected_tuples[0])


@pytest.mark.parametrize('table', ['MUTATIONS', 'CORE_MUTATIONS', 'DAEMON_MUTATIONS'])
def test_baseline_group_plan_preserves_every_current_registry_filter(monkeypatch, tmp_path, table):
    """Inspect actual registered selectors; mocked terminals are not native evidence."""
    mutations = getattr(gate, table)
    calls = []
    monkeypatch.setattr(gate, 'build', lambda *args: gate.Step(status='pass'))
    def run_step(args, target, mutation, filters, seeds, timeout, log):
        calls.append((tuple(filters), seeds, timeout))
        return gate.Step(status='pass', selected=list(filters), ran=list(filters))
    monkeypatch.setattr(gate, 'run_step', run_step)
    result = gate.evaluate_baseline(synthetic_scheduling_args(tmp_path), tmp_path, mutations)
    named = [filters for filters, seeds, _ in calls if seeds is None]
    scenarios = [filters for filters, seeds, _ in calls if seeds is not None]
    assert named == sorted({mu.tests for mu in mutations})
    assert {f for filters in named for f in filters} == {t for mu in mutations for t in mu.tests}
    assert scenarios == [(s,) for s in sorted({gate.SCENARIOS[s] for mu in mutations for s in mu.scenarios})]
    assert result['verdict'] == 'pass'
    assert all(cap == (900 if seeds is None else 3600) for _, seeds, cap in calls)


@pytest.mark.parametrize('bad_status', ['timeout', 'missing-test', 'execution-error', 'build-error', 'fail'])
def test_any_baseline_group_refusal_survives_later_pass_and_preserves_union(monkeypatch, tmp_path, bad_status):
    mutations = [gate.m('SYNTHETIC_A', 'fixture', ['tests::a']),
                 gate.m('SYNTHETIC_B', 'fixture', ['tests::b'])]
    calls = []
    monkeypatch.setattr(gate, 'build', lambda *args: gate.Step(status='pass'))
    def run_step(args, target, mutation, filters, seeds, timeout, log):
        calls.append(tuple(filters))
        status = bad_status if len(calls) == 1 else 'pass'
        return gate.Step(status=status, selected=list(filters), ran=list(filters),
                         failed=list(filters) if status == 'fail' else [])
    monkeypatch.setattr(gate, 'run_step', run_step)
    result = gate.evaluate_baseline(synthetic_scheduling_args(tmp_path), tmp_path, mutations)
    assert calls == [('tests::a',), ('tests::b',)]
    assert result['verdict'] == 'fail'
    assert result['named']['status'] == bad_status
    assert [step['status'] for step in result['named']['steps']] == [bad_status, 'pass']
    assert result['named']['selected'] == ['tests::a', 'tests::b']


def test_grouped_discovery_timeout_never_launches_or_replenishes_that_step(monkeypatch, tmp_path):
    monkeypatch.setattr(gate.time, 'monotonic', lambda: 100.0)
    monkeypatch.setattr(gate, 'build', lambda *args: gate.Step(status='pass'))
    calls = []
    def cargo_test(args, target, mutation, filters, seeds, timeout, log, no_run=False):
        calls.append((tuple(filters), timeout))
        if filters[0] == 'tests::a':
            assert '--list' in filters, 'an exhausted discovery cannot launch runtime'
            return 0, 'tests::a: test\n\n1 test, 0 benchmarks\n', 900.0
        if '--list' in filters:
            return 0, 'tests::b: test\n\n1 test, 0 benchmarks\n', 1.0
        return 0, ('running 1 test\ntest tests::b ... ok\ntest result: ok. 1 passed; 0 failed; '
                   '0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'), 1.0
    monkeypatch.setattr(gate, 'cargo_test', cargo_test)
    mutations = [gate.m('SYNTHETIC_A', 'fixture', ['tests::a']),
                 gate.m('SYNTHETIC_B', 'fixture', ['tests::b'])]
    result = gate.evaluate_baseline(synthetic_scheduling_args(tmp_path), tmp_path, mutations)
    assert result['verdict'] == 'fail' and result['named']['status'] == 'timeout'
    assert len(calls) == 3
    assert [timeout for _, timeout in calls] == [900, 900, 899]
    assert [step['status'] for step in result['named']['steps']] == ['timeout', 'pass']
    assert result['named']['steps'][0]['ran'] == []
    assert result['named']['steps'][1]['ran'] == ['tests::b']


@pytest.mark.parametrize('output,code', [
    ('running 1 test\ntest tests::a ... ok\n', 0),
    ('running 1 test\ntest tests::a ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s\n', 0),
    ('running 1 test\ntest tests::a ... ok\ntest tests::a ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n', 0),
    ('running 1 test\ntest tests::foreign ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n', 0),
])
def test_grouped_baseline_requires_exact_actual_discovered_terminals(monkeypatch, tmp_path, output, code):
    monkeypatch.setattr(gate, 'build', lambda *args: gate.Step(status='pass'))
    monkeypatch.setattr(gate.time, 'monotonic', lambda: 100.0)
    def cargo_test(args, target, mutation, filters, seeds, timeout, log, no_run=False):
        if '--list' in filters:
            return 0, 'tests::a: test\n\n1 test, 0 benchmarks\n', 0.0
        return code, output, 1.0
    monkeypatch.setattr(gate, 'cargo_test', cargo_test)
    result = gate.evaluate_baseline(synthetic_scheduling_args(tmp_path), tmp_path,
                                    [gate.m('SYNTHETIC_A', 'fixture', ['tests::a'])])
    assert result['verdict'] == 'fail'
    assert result['named']['status'] not in ('pass', 'fail')
    assert len(result['named']['steps']) == 1
    assert result['named']['steps'][0]['selected'] == ['tests::a']


def test_mutant_preserves_named_tuple_and_runs_each_distinct_scenario_once(monkeypatch, tmp_path):
    scenarios = list(gate.SCENARIOS)[:2]
    mu = gate.m('SYNTHETIC_A', 'fixture', ['tests::b', 'tests::a'],
                [scenarios[1], scenarios[0], scenarios[1]])
    calls, builds = [], []
    monkeypatch.setattr(gate, 'has_switch', lambda *args, **kwargs: True)
    monkeypatch.setattr(gate, 'build', lambda *args: builds.append(args) or gate.Step(status='pass'))
    def run_step(args, target, mutation, filters, seeds, timeout, log):
        calls.append((mutation, tuple(filters), seeds, timeout, log))
        return gate.Step(status='fail' if seeds is None else 'pass',
                         failed=['tests::a'] if seeds is None else [], selected=list(filters), ran=list(filters))
    monkeypatch.setattr(gate, 'run_step', run_step)
    result = gate.evaluate(synthetic_scheduling_args(tmp_path), tmp_path, mu)
    assert result['verdict'] == 'killed_by_test'
    assert len(builds) == 1 and builds[0][2] == mu.id
    assert calls[0][1:4] == (mu.tests, None, 900)
    assert [call[1:4] for call in calls[1:]] == [
        ((gate.SCENARIOS[scenarios[1]],), 200, 3600),
        ((gate.SCENARIOS[scenarios[0]],), 200, 3600)]
    assert all(call[0] == mu.id for call in calls)
    assert len({call[4] for call in calls}) == 3
    assert result['named']['failed'] == ['tests::a']
    assert result['scenario']['status'] == 'pass'
    assert len(result['scenario']['steps']) == 2


@pytest.mark.parametrize('bad_status', ['timeout', 'missing-test', 'execution-error', 'build-error'])
def test_mutant_scenario_error_cannot_be_hidden_by_named_or_earlier_scenario_kill(monkeypatch, tmp_path, bad_status):
    scenarios = list(gate.SCENARIOS)[:2]
    mu = gate.m('SYNTHETIC_A', 'fixture', ['tests::named'], scenarios)
    monkeypatch.setattr(gate, 'has_switch', lambda *args, **kwargs: True)
    monkeypatch.setattr(gate, 'build', lambda *args: gate.Step(status='pass'))
    steps = iter([gate.Step(status='fail', failed=['tests::named']),
                  gate.Step(status='fail', failed=[gate.SCENARIOS[scenarios[0]]]),
                  gate.Step(status=bad_status)])
    monkeypatch.setattr(gate, 'run_step', lambda *args: next(steps))
    result = gate.evaluate(synthetic_scheduling_args(tmp_path), tmp_path, mu)
    assert result['verdict'] == 'error' and result['reason'] == f'scenarios: {bad_status}'
    assert result['scenario']['status'] == bad_status
    assert len(result['scenario']['steps']) == 2
    assert result['scenario']['failed'] == [gate.SCENARIOS[scenarios[0]]]


def test_independent_steps_may_exceed_union_budget_without_replenishing_a_step(monkeypatch, tmp_path):
    """The prospective policy is per invocation; no historical timeout is reclassified."""
    monkeypatch.setattr(gate, 'build', lambda *args: gate.Step(status='pass'))
    calls = []
    def run_step(args, target, mutation, filters, seeds, timeout, log):
        calls.append(timeout)
        return gate.Step(status='pass', seconds=800.0, selected=list(filters), ran=list(filters))
    monkeypatch.setattr(gate, 'run_step', run_step)
    mutations = [gate.m('SYNTHETIC_A', 'fixture', ['tests::a']),
                 gate.m('SYNTHETIC_B', 'fixture', ['tests::b'])]
    result = gate.evaluate_baseline(synthetic_scheduling_args(tmp_path), tmp_path, mutations)
    assert calls == [900, 900]
    assert result['verdict'] == 'pass' and result['named']['seconds'] == 1600.0
    assert [step['deadline_seconds'] for step in result['named']['steps']] == [900, 900]


@pytest.mark.parametrize('fast', [False, True])
def test_baseline_and_mutant_fast_mode_preserve_named_coverage_only(monkeypatch, tmp_path, fast):
    scenario = next(iter(gate.SCENARIOS))
    mu = gate.m('SYNTHETIC_A', 'fixture', ['tests::b', 'tests::a'], [scenario])
    calls = []
    monkeypatch.setattr(gate, 'build', lambda *args: gate.Step(status='pass'))
    monkeypatch.setattr(gate, 'has_switch', lambda *args, **kwargs: True)
    def run_step(args, target, mutation, filters, seeds, timeout, log):
        calls.append((mutation, tuple(filters), seeds, timeout))
        return gate.Step(status='pass')
    monkeypatch.setattr(gate, 'run_step', run_step)
    args = synthetic_scheduling_args(tmp_path, fast=fast)
    baseline = gate.evaluate_baseline(args, tmp_path, [mu])
    mutant = gate.evaluate(args, tmp_path, mu)
    assert len(calls) == (2 if fast else 4)
    assert [filters for _, filters, seeds, _ in calls if seeds is None] == [mu.tests, mu.tests]
    assert ('scenario' in baseline) == ('scenario' in mutant) == (not fast)


@pytest.mark.parametrize('named_status,scenario_statuses,strict,expected,verdict', [
    ('pass', ('fail', 'pass'), False, 0, 'killed_by_scenario_only'),
    ('pass', ('fail', 'pass'), True, 1, 'killed_by_scenario_only'),
    ('fail', ('pass', 'pass'), True, 0, 'killed_by_test'),
    ('fail', ('fail', 'timeout'), True, 1, 'error'),
    ('pass', ('pass', 'pass'), False, 1, 'survived'),
])
def test_main_report_preserves_strict_and_error_semantics_of_grouped_scenarios(
    monkeypatch, tmp_path, named_status, scenario_statuses, strict, expected, verdict
):
    """Synthetic in-memory registry and mocked terminals are not native evidence."""
    scenarios = list(gate.SCENARIOS)[:2]
    mu = gate.m('SYNTHETIC_A', 'fixture', ['tests::named'], scenarios)
    monkeypatch.setattr(gate, 'MUTATIONS', [mu])
    monkeypatch.setattr(gate, 'has_switch', lambda *args, **kwargs: True)
    monkeypatch.setattr(gate, 'build', lambda *args: gate.Step(status='pass'))
    def run_step(args, target, mutation, filters, seeds, timeout, log):
        if mutation is None:
            status = 'pass'
        elif seeds is None:
            status = named_status
        else:
            status = scenario_statuses[scenarios.index(next(s for s in scenarios if gate.SCENARIOS[s] == filters[0]))]
        return gate.Step(status=status, selected=list(filters), ran=list(filters),
                         failed=list(filters) if status == 'fail' else [])
    monkeypatch.setattr(gate, 'run_step', run_step)
    monkeypatch.setattr(sys, 'argv', ['sumeragi_mutation_gate.py', '--jobs', '1',
                                    '--target-dir', str(tmp_path), *(['--strict'] if strict else [])])
    assert gate.main() == expected
    report = gate.json.loads((tmp_path / 'report.json').read_text())
    assert report['baseline']['verdict'] == 'pass'
    assert report['mutations'][0]['verdict'] == verdict
    assert len(report['mutations'][0]['scenario']['steps']) == 2
    assert report['summary']['scenario_missed'] == ([mu.id] if scenario_statuses == ('pass', 'pass') else [])
    assert report['summary']['error'] == ([mu.id] if verdict == 'error' else [])


def test_grouped_substring_discovery_preserves_extra_and_overlapping_actual_names(monkeypatch, tmp_path):
    monkeypatch.setattr(gate, 'build', lambda *args: gate.Step(status='pass'))
    monkeypatch.setattr(gate.time, 'monotonic', lambda: 100.0)
    actual_names = ['tests::named', 'tests::named_extra', 'tests::other']
    runtime = []
    def cargo_test(args, target, mutation, filters, seeds, timeout, log, no_run=False):
        selected = [name for name in actual_names if any(f in name for f in filters if not f.startswith('--'))]
        if '--list' in filters:
            return 0, ''.join(f'{name}: test\n' for name in selected) + f'\n{len(selected)} tests, 0 benchmarks\n', 0.0
        runtime.append(selected)
        return 0, (f'running {len(selected)} tests\n' + ''.join(f'test {name} ... ok\n' for name in selected)
                   + f'test result: ok. {len(selected)} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'), 1.0
    monkeypatch.setattr(gate, 'cargo_test', cargo_test)
    mutations = [gate.m('SYNTHETIC_A', 'fixture', ['tests::named']),
                 gate.m('SYNTHETIC_B', 'fixture', ['tests::named_extra']),
                 gate.m('SYNTHETIC_C', 'fixture', ['tests::other'])]
    result = gate.evaluate_baseline(synthetic_scheduling_args(tmp_path), tmp_path, mutations)
    assert result['verdict'] == 'pass'
    assert runtime == [['tests::named', 'tests::named_extra'], ['tests::named_extra'], ['tests::other']]
    assert result['named']['selected'] == result['named']['ran'] == actual_names
    assert [step['selected'] for step in result['named']['steps']] == runtime


def test_mutant_scenario_runtime_cannot_reset_discovery_budget_or_hide_late_pass(monkeypatch, tmp_path):
    scenarios = list(gate.SCENARIOS)[:2]
    mu = gate.m('SYNTHETIC_A', 'fixture', ['tests::named'], scenarios)
    monkeypatch.setattr(gate, 'has_switch', lambda *args, **kwargs: True)
    monkeypatch.setattr(gate, 'build', lambda *args: gate.Step(status='pass'))
    monkeypatch.setattr(gate.time, 'monotonic', lambda: 100.0)
    calls = []
    def cargo_test(args, target, mutation, filters, seeds, timeout, log, no_run=False):
        assert len([f for f in filters if f == 'tests::named' or f in (gate.SCENARIOS[s] for s in scenarios)]) == 1
        name = filters[0]
        calls.append((name, '--list' in filters, timeout))
        if '--list' in filters:
            return 0, f'{name}: test\n\n1 test, 0 benchmarks\n', 2.0
        late = name == gate.SCENARIOS[scenarios[1]]
        verdict = 'ok' if late else 'FAILED'
        output = (f'running 1 test\ntest {name} ... {verdict}\ntest result: {verdict}. '
                  f'{1 if late else 0} passed; {0 if late else 1} failed; '
                  '0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n')
        return (0 if late else 101), output, (3598.5 if late else 1.0)
    monkeypatch.setattr(gate, 'cargo_test', cargo_test)
    result = gate.evaluate(synthetic_scheduling_args(tmp_path), tmp_path, mu)
    assert result['named']['status'] == 'fail'
    assert result['verdict'] == 'error' and result['reason'] == 'scenarios: timeout'
    assert [step['status'] for step in result['scenario']['steps']] == ['fail', 'timeout']
    assert [timeout for _, _, timeout in calls] == [900, 898, 3600, 3598, 3600, 3598]
    assert result['scenario']['steps'][1]['ran'] == [gate.SCENARIOS[scenarios[1]]]


@pytest.mark.parametrize('groups', [[], [()]])
def test_empty_group_or_selector_tuple_refuses_without_unfiltered_native_run(monkeypatch, tmp_path, groups):
    def no_step(*args):
        raise AssertionError('an empty selector cannot spawn a broad unfiltered suite')
    monkeypatch.setattr(gate, 'run_step', no_step)
    result = gate.run_grouped_steps(synthetic_scheduling_args(tmp_path), tmp_path, None,
                                   groups, None, 900, tmp_path, 'synthetic-empty')
    assert result['status'] == 'missing-test'
    assert result['selected'] == result['ran'] == result['failed'] == []
    assert len(result['steps']) == len(groups)
    if groups:
        assert result['steps'][0]['detail'] == ['no selectors for this invocation']
        assert result['steps'][0]['status'] == 'missing-test'
        assert result['steps'][0]['discovery_log'] == ''


def test_empty_mutant_named_tuple_refuses_before_source_build_or_unfiltered_run(monkeypatch, tmp_path):
    def no_operation(*args, **kwargs):
        raise AssertionError('a mutation without named controls cannot inspect/build/run an unfiltered suite')
    for name in ('has_switch', 'build', 'run_step'):
        monkeypatch.setattr(gate, name, no_operation)
    result = gate.evaluate(synthetic_scheduling_args(tmp_path), tmp_path,
                           gate.m('SYNTHETIC_EMPTY', 'fixture', [], [next(iter(gate.SCENARIOS))]))
    assert result['verdict'] == 'error' and result['reason'] == 'no named test selectors'
    assert 'build' not in result and 'named' not in result and 'scenario' not in result


@pytest.mark.parametrize("extra,message", [
    (["--seeds", "0"], "positive u64 count"),
    (["--seeds", "-1"], "positive u64 count"),
    (["--seeds", str(1 << 64)], "positive u64 count"),
    (["--seeds", str(-(1 << 64))], "positive u64 count"),
    (["--jobs", "0"], "must be positive"),
    (["--jobs", "-1"], "must be positive"),
    (["--only", ""], "at least one mutation"),
    (["--only", " , , "], "at least one mutation"),
    (["--only", "MS1,MS1"], "duplicate mutation"),
    (["--only", " MS1, MS1 "], "duplicate mutation"),
])
@pytest.mark.parametrize("strict", [False, True])
def test_mutation_cli_invalid_counts_refuse_before_output_or_native_child(monkeypatch, tmp_path, capsys, extra, message, strict):
    output = tmp_path / "unused-campaign"
    def forbidden(*args, **kwargs):
        pytest.fail("invalid counts reached execution")
    monkeypatch.setattr(gate, "evaluate_baseline", forbidden)
    monkeypatch.setattr(gate, "evaluate", forbidden)
    monkeypatch.setattr(gate.subprocess, "Popen", forbidden)
    monkeypatch.setattr(sys, "argv", ["gate", "--target-dir", str(output), *(["--strict"] if strict else []), *extra])
    with pytest.raises(SystemExit) as error:
        gate.main()
    assert error.value.code == 2
    assert message in capsys.readouterr().err
    assert not output.exists()

@pytest.mark.parametrize("seeds,jobs", [(1,1),(200,4),(10000,2),((1 << 64)-1,1)])
def test_mutation_cli_valid_counts_and_unique_selection_keep_exact_report(monkeypatch, tmp_path, seeds, jobs):
    import json
    called=[]
    def baseline(args, target, mutations):
        called.append((args.seeds, args.jobs, [m.id for m in mutations]))
        return {"id":"baseline", "verdict":"pass"}
    monkeypatch.setattr(gate,"evaluate_baseline",baseline)
    monkeypatch.setattr(gate,"evaluate",lambda args,target,mu:{"id":mu.id,"verdict":"killed_by_test","named":{"failed":["tests::named"]}})
    monkeypatch.setattr(sys,"argv",["gate","--strict","--only"," MS1, MS2 ","--seeds",str(seeds),"--jobs",str(jobs),"--target-dir",str(tmp_path)])
    assert gate.main()==0
    assert called==[(seeds,jobs,["MS1","MS2"])]
    report=json.loads((tmp_path/"report.json").read_text())
    assert report["summary"]["mutations"]==2
    assert report["summary"]["killed_by_test"]==["MS1","MS2"]
    assert [m["id"] for m in report["mutations"]]==["MS1","MS2"]
    assert report["seeds"]==seeds
    assert report["fast"] is False


def test_state_native_descriptor_mutation_binds_exact_admitted_engine_and_original_retry():
    rule = gate.index_mutations(gate.CORE_MUTATIONS)["HC146"]
    assert rule.tests == (
        "state::state_preverify_backend_admission_tests::unsupported_retired_and_claimed_backends_fail_state_admission",
        "state::state_preverify_backend_admission_tests::native_compiled_descriptor_refusal_preserves_key_admission_and_original_retry",
    )
    assert not rule.scenarios
    assert gate.has_switch("HC146", core=True)
    assert not gate.has_switch("HC146")
    assert not gate.has_switch("HC146", daemon=True)
    source = (gate.REPO / "crates/iroha_core/src/state.rs").read_text()
    assert 'all(test, sumeragi_core_mutation = "HC146")' in source
    assert 'production_verify_backend_tag(proof.backend.as_str())' in source
    assert 'Some(iroha_data_model::zk::BackendTag::NativePipaRPasta)' in source
    owners = {path.relative_to(gate.REPO / "crates/iroha_core/src").as_posix()
              for path in (gate.REPO / "crates/iroha_core/src").rglob("*.rs")
              if 'sumeragi_core_mutation = "HC146"' in path.read_text()}
    assert owners == {"state.rs"}
