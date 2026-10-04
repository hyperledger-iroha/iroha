"""Fresh-process regressions for independent canonical script-test imports."""

from __future__ import annotations

from pathlib import Path
import re
import subprocess
import sys

import pytest


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
RECEIPT_TEST = REPOSITORY_ROOT / "scripts/tests/sorafs_software_signer_receipt_validation_test.py"
CACHE_TEST = REPOSITORY_ROOT / "scripts/tests/sorafs_resilience_fixture_key_cache_test.py"


@pytest.mark.parametrize("order,expected_cases", (
    ("receipt-only", 8),
    ("receipt-first", 11),
    ("cache-first", 11),
    ("inert-first", 9),
))
def test_script_modules_import_and_execute_in_a_fresh_process(
    tmp_path: Path, order: str, expected_cases: int,
) -> None:
    """Run real original units from an unrelated cwd with isolated import state."""
    if order == "receipt-only":
        selected = [RECEIPT_TEST]
    elif order == "receipt-first":
        selected = [RECEIPT_TEST, CACHE_TEST]
    elif order == "cache-first":
        selected = [CACHE_TEST, RECEIPT_TEST]
    else:
        inert = tmp_path / "test_inert.py"
        inert.write_text("def test_inert():\n    assert 2 + 2 == 4\n", encoding="utf-8")
        selected = [inert, RECEIPT_TEST]
    completed = subprocess.run(
        [sys.executable, "-I", "-B", "-m", "pytest", "-q",
         "-p", "no:cacheprovider", "--basetemp=" + str(tmp_path / "child-tmp"),
         *map(str, selected)],
        cwd=tmp_path, capture_output=True, text=True, timeout=60, check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    summary = re.search(r"(?:^|\s)(\d+) passed(?:,| in|$)", completed.stdout)
    assert summary is not None, completed.stdout
    assert int(summary.group(1)) == expected_cases, completed.stdout
