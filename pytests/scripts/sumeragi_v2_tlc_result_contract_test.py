"""Execute TLC result guards and reject resealed helper/caller bypasses."""

from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
HELPER = "scripts/formal/sumeragi_v2_tlc_result_contract.sh"
CANDIDATE = "scripts/formal/run_sumeragi_v2_candidate_restart_mutation.sh"
INFLIGHT = "scripts/formal/run_sumeragi_v2_inflight_first_release.sh"
FIXED = "sumeragi_v2_tlc_assert_fixed_success"
ACTION = "sumeragi_v2_tlc_assert_action_property_violation"
SUMMARY = "1,234 states generated, 567 distinct states found, 0 states left on queue."
FOOTER = "Finished in 1s at (2026-09-21 10:20:30)"
SUCCESS = "Model checking completed. No error has been found."
VIOLATION = "Error: Action property ExactOwner is violated."
BEHAVIOR = "Error: The behavior up to this point is:"


@pytest.fixture(scope="module")
def checker():
    path = ROOT / "scripts/formal/check_sumeragi_v2_proof_ledger.py"
    spec = importlib.util.spec_from_file_location("tlc_result_contract_test_checker", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def source_tree(tmp_path, checker):
    for relative in checker.SHARED_TLC_RESULT_CONTRACT_SHA256:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, destination)
    assert checker._shared_tlc_result_contract_source_fidelity_errors(tmp_path) == []
    return tmp_path


def mutate_reseal(source_tree, checker, monkeypatch, relative, old, new):
    path = source_tree / relative
    source = path.read_text()
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1))
    monkeypatch.setitem(
        checker.SHARED_TLC_RESULT_CONTRACT_SHA256,
        relative,
        hashlib.sha256(path.read_bytes()).hexdigest(),
    )
    return checker._shared_tlc_result_contract_source_fidelity_errors(source_tree)


@pytest.mark.parametrize("owner", (FIXED, ACTION))
@pytest.mark.parametrize(
    ("assertion", "diagnostic"),
    (
        ("sumeragi_v2_tlc_assert_nonzero_state_space", "nonzero-state assertion"),
        ("sumeragi_v2_tlc_assert_terminal", "terminal-footer assertion"),
    ),
)
def test_helper_assertions_cannot_be_borrowed_from_adjacent_owner(
    source_tree, checker, monkeypatch, owner, assertion, diagnostic
):
    path = source_tree / HELPER
    source = path.read_text()
    start = source.index(owner + "() {")
    end = source.index("\n}\n", start) + 3
    section = source[start:end]
    old = f'  {assertion} "$label" "$log"\n'
    assert section.count(old) == 1
    errors = mutate_reseal(
        source_tree, checker, monkeypatch, HELPER,
        section, section.replace(old, "", 1),
    )
    expected_owner = "fixed-success" if owner == FIXED else "action-property"
    assert any(expected_owner in error and diagnostic in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new", "diagnostic"),
    (
        ('[[ "$actual_status" -eq 13 ]] || {', '[[ "$actual_status" -eq 0 ]] || {', 'exact status-13 guard'),
        ('[[ "$failure_count" == 2 ]] || {', '[[ "$failure_count" -ge 2 ]] || {', 'exact primary-plus-behavior diagnostic guard'),
        ('[[ "$primary_diagnostic_count" == 1 ]] || {', '[[ "$primary_diagnostic_count" -ge 1 ]] || {', 'single primary-diagnostic guard'),
        ('"$label" "$log" "$expected_marker"', '"$label" "$log" "$SUMERAGI_V2_TLC_SUCCESS_MARKER"', 'exact action-property marker assertion'),
        ('"$label" "$log" "$SUMERAGI_V2_TLC_VIOLATION_BEHAVIOR_MARKER"', '"$label" "$log" "$expected_marker"', 'exact violation-behavior marker assertion'),
        ('^Error:\\ Action\\ property\\ .+\\ is\\ violated\\.$', '^Error:.*', 'canonical action-property marker guard'),
        ('if ((generated <= 0 || distinct <= 0)); then', 'if ((generated < 0 || distinct < 0)); then', 'positive generated/distinct state-count guard'),
        ('[[ "$terminal_count" == 1 ]] || {', '[[ "$terminal_count" -ge 1 ]] || {', 'single terminal-marker cardinality guard'),
        ('<<<"$last_nonblank" || {', '"$log" || {', 'terminal footer position guard'),
    ),
)
def test_resealed_guard_weakening_still_fails(source_tree, checker, monkeypatch, old, new, diagnostic):
    errors = mutate_reseal(source_tree, checker, monkeypatch, HELPER, old, new)
    assert any(diagnostic in error for error in errors), errors


@pytest.mark.parametrize("relative", (CANDIDATE, INFLIGHT))
@pytest.mark.parametrize("assertion", (
    "sumeragi_v2_tlc_assert_fixed_success", "sumeragi_v2_tlc_assert_nonzero_state_space",
    "sumeragi_v2_tlc_assert_terminal",
))
def test_resealed_runner_cannot_bypass_shared_assertion(source_tree, checker, monkeypatch, relative, assertion):
    errors = mutate_reseal(source_tree, checker, monkeypatch, relative, assertion, "removed_assertion")
    assert any("assertion-site profile must equal" in error for error in errors), errors


@pytest.mark.parametrize("relative, call", (
    (CANDIDATE, 'run_green identity-exact "$IDENTITY_MODULE" candidate_identity_exact.cfg'),
    (CANDIDATE, 'run_mutant ingress-class-runtime-chunk "$INGRESS_CLASS_MODULE"'),
    (INFLIGHT, "\nrun_positive\n"),
    (INFLIGHT, 'run_mutant inflight_first_release_direct_release_with_active_kura_bug.cfg MLDirectReleaseRequiresAbsentKura'),
))
def test_resealed_runner_cannot_drop_a_reviewed_case(source_tree, checker, monkeypatch, relative, call):
    errors = mutate_reseal(source_tree, checker, monkeypatch, relative, call, "\n# removed case\n")
    assert any("reviewed corpus branch counts" in error for error in errors), errors


def test_resealed_inflight_cannot_understate_retained_case_count(source_tree, checker, monkeypatch):
    errors = mutate_reseal(source_tree, checker, monkeypatch, INFLIGHT, "--expected-cases 26", "--expected-cases 25")
    assert any("expected-case count" in error for error in errors), errors


def test_unreviewed_helper_function_is_not_folded_into_another_owner(source_tree, checker, monkeypatch):
    errors = mutate_reseal(
        source_tree, checker, monkeypatch, HELPER,
        ACTION + "() {", "sumeragi_v2_tlc_unreviewed() {\n  :\n}\n\n" + ACTION + "() {",
    )
    assert any("ordered helper-function inventory" in error for error in errors), errors


def invoke(tmp_path, owner, lines, status, marker=VIOLATION, *, symlink=False):
    log = tmp_path / "tlc.log"
    log.write_text("\n".join(lines) + "\n")
    if symlink:
        actual = tmp_path / "actual.log"
        log.rename(actual)
        log.symlink_to(actual)
    return subprocess.run(
        ["bash", "-c", 'set -euo pipefail; source "$1"; "$2" control "$3" "$4" "$5"',
         "tlc-result-control", str(ROOT / HELPER), owner, str(log), str(status), marker],
        text=True, capture_output=True, timeout=10,
    )


@pytest.mark.parametrize("owner,status,markers", ((FIXED, 0, [SUCCESS]), (ACTION, 13, [VIOLATION, BEHAVIOR])))
@pytest.mark.parametrize("footer", (
    FOOTER, "Finished in 123ms at (2026-09-21 10:20:30)",
    "Finished in 1d 2h 3min 4s at (2026-09-21 10:20:30)",
    "Finished in 2min at (2026-09-21 10:20:30)",
))
def test_real_shell_contract_accepts_reviewed_transcript_grammar(tmp_path, owner, status, markers, footer):
    result = invoke(tmp_path, owner, markers + [SUMMARY, footer, "", "  "], status)
    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize("owner,status,markers", ((FIXED, 0, [SUCCESS]), (ACTION, 13, [VIOLATION, BEHAVIOR])))
@pytest.mark.parametrize("mutation", (
    "missing_summary", "zero_generated", "zero_distinct", "malformed_summary", "last_summary_zero",
    "missing_footer", "duplicate_footer", "trailing_output", "embedded_footer", "prefixed_footer",
    "wrong_status", "missing_marker", "duplicate_marker", "unexpected_diagnostic", "symlink",
))
def test_real_shell_contract_rejects_invalid_result(tmp_path, owner, status, markers, mutation):
    lines = markers + [SUMMARY, FOOTER]
    symlink = False
    if mutation == "missing_summary": lines.remove(SUMMARY)
    elif mutation == "zero_generated": lines[-2] = SUMMARY.replace("1,234", "0")
    elif mutation == "zero_distinct": lines[-2] = SUMMARY.replace("567", "0")
    elif mutation == "malformed_summary": lines[-2] = SUMMARY + " ignored suffix"
    elif mutation == "last_summary_zero": lines.insert(-1, SUMMARY.replace("567", "0"))
    elif mutation == "missing_footer": lines.pop()
    elif mutation == "duplicate_footer": lines.append(FOOTER)
    elif mutation == "trailing_output": lines.append("unexpected trailing output")
    elif mutation == "embedded_footer": lines[-1] = "prefix " + FOOTER + " suffix"
    elif mutation == "prefixed_footer": lines[-1] = " " + FOOTER
    elif mutation == "wrong_status": status = 12
    elif mutation == "missing_marker": lines.pop(0)
    elif mutation == "duplicate_marker": lines.insert(0, lines[0])
    elif mutation == "unexpected_diagnostic": lines.insert(-1, "  Error: unrelated failure")
    elif mutation == "symlink": symlink = True
    else: raise AssertionError(mutation)
    result = invoke(tmp_path, owner, lines, status, symlink=symlink)
    assert result.returncode != 0, result.stdout


@pytest.mark.parametrize("mutation", ("no_behavior", "wrong_kind", "extra_primary"))
def test_action_property_failure_requires_exact_diagnostic_contract(tmp_path, mutation):
    marker = VIOLATION
    lines = [VIOLATION, BEHAVIOR, SUMMARY, FOOTER]
    if mutation == "no_behavior": lines.remove(BEHAVIOR)
    elif mutation == "wrong_kind":
        marker = "Error: Invariant ExactOwner is violated."
        lines[0] = marker
    elif mutation == "extra_primary": lines.insert(1, "Error: Action property OtherOwner is violated.")
    result = invoke(tmp_path, ACTION, lines, 13, marker)
    assert result.returncode != 0, result.stdout
