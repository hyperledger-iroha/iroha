"""Execute the shared TLC result guards against accepted and rejected transcripts."""

from __future__ import annotations

from pathlib import Path
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[2]
HELPER = "scripts/formal/tlc_result_contract.sh"
FIXED = "tlc_assert_fixed_success"
ACTION = "tlc_assert_action_property_violation"
SUMMARY = "1,234 states generated, 567 distinct states found, 0 states left on queue."
FOOTER = "Finished in 1s at (2026-09-21 10:20:30)"
SUCCESS = "Model checking completed. No error has been found."
VIOLATION = "Error: Action property ExactOwner is violated."
BEHAVIOR = "Error: The behavior up to this point is:"


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
