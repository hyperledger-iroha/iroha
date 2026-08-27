"""Tests for the fail-closed SORA Parliament crypto benchmark evidence gate."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
CHECKER_PATH = REPOSITORY_ROOT / "scripts/check_sora_parliament_crypto_bench.py"
SPEC = importlib.util.spec_from_file_location("parliament_crypto_bench_checker", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


def _write_criterion(root: Path) -> None:
    for index, benchmark_id in enumerate(checker.EXPECTED_BENCHMARK_IDS):
        sample = root / f"sanitized-case-{index:02}" / "new"
        sample.mkdir(parents=True)
        (sample / "benchmark.json").write_text(
            json.dumps({"full_id": benchmark_id}, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        (sample / "estimates.json").write_text(
            json.dumps(
                {"median": {"point_estimate": 1_000 + index}}, sort_keys=True
            )
            + "\n",
            encoding="utf-8",
        )


def _write_allocations(path: Path, *, first_value: str = "1") -> None:
    lines = [
        f"schema\t{checker.ALLOCATION_SCHEMA}",
        f"scope\t{checker.ALLOCATION_SCOPE}",
        "benchmark_id\t" + "\t".join(checker.ALLOCATION_COLUMNS),
    ]
    for index, benchmark_id in enumerate(checker.EXPECTED_BENCHMARK_IDS):
        allocation_calls = first_value if index == 0 else "1"
        lines.append(f"{benchmark_id}\t{allocation_calls}\t0\t64\t0")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


@pytest.fixture
def evidence(tmp_path: Path) -> tuple[Path, Path, str]:
    criterion = tmp_path / "criterion"
    allocations = tmp_path / "allocations.tsv"
    _write_criterion(criterion)
    _write_allocations(allocations)
    return criterion, allocations, checker.checkout_commit(REPOSITORY_ROOT)


def _report(evidence: tuple[Path, Path, str]) -> dict[str, object]:
    criterion, allocations, commit = evidence
    return checker.build_report(
        criterion_dir=criterion,
        allocation_evidence=allocations,
        repository_root=REPOSITORY_ROOT,
        expected_source_commit=commit,
    )


def test_complete_inventory_produces_deterministic_hashed_report(
    evidence: tuple[Path, Path, str],
) -> None:
    first = _report(evidence)
    second = _report(evidence)
    assert checker.canonical_report_bytes(first) == checker.canonical_report_bytes(second)
    assert first["schema"] == checker.REPORT_SCHEMA
    assert first["successful"] is True
    assert first["budget_policy"] == {
        "allocation_ceilings_enforced": False,
        "latency_ceilings_enforced": False,
    }
    assert len(first["inputs"]["source"]) == len(checker.SOURCE_INPUT_PATHS)
    assert set(first["measurements"]["allocations"]) == set(
        checker.EXPECTED_BENCHMARK_IDS
    )
    assert len(first["integrity"]["closure_sha256"]) == 64


def test_report_verification_rejects_raw_evidence_tampering(
    evidence: tuple[Path, Path, str], tmp_path: Path
) -> None:
    report = _report(evidence)
    report_path = tmp_path / "report.json"
    checker.write_report(report_path, report)
    checker.verify_report(report_path, _report(evidence))

    criterion, _, _ = evidence
    estimates = criterion / "sanitized-case-00" / "new" / "estimates.json"
    estimates.write_text(
        json.dumps({"median": {"point_estimate": 9_999}}) + "\n",
        encoding="utf-8",
    )
    with pytest.raises(checker.EvidenceError, match="stale, or tampered"):
        checker.verify_report(report_path, _report(evidence))


def test_missing_or_extra_criterion_case_is_rejected(
    evidence: tuple[Path, Path, str]
) -> None:
    criterion, _, _ = evidence
    (criterion / "sanitized-case-00" / "new" / "benchmark.json").unlink()
    with pytest.raises(checker.EvidenceError, match="inventory mismatch"):
        _report(evidence)

    extra = criterion / "extra" / "new"
    extra.mkdir(parents=True)
    (extra / "benchmark.json").write_text(
        json.dumps({"full_id": "parliament/timed_ovn/extra"}) + "\n",
        encoding="utf-8",
    )
    (extra / "estimates.json").write_text(
        json.dumps({"median": {"point_estimate": 1}}) + "\n",
        encoding="utf-8",
    )
    with pytest.raises(checker.EvidenceError, match="inventory mismatch"):
        _report(evidence)


def test_duplicate_criterion_identity_is_rejected(
    evidence: tuple[Path, Path, str]
) -> None:
    criterion, _, _ = evidence
    duplicate = criterion / "duplicate" / "new"
    duplicate.mkdir(parents=True)
    (duplicate / "benchmark.json").write_text(
        json.dumps({"full_id": checker.EXPECTED_BENCHMARK_IDS[0]}) + "\n",
        encoding="utf-8",
    )
    (duplicate / "estimates.json").write_text(
        json.dumps({"median": {"point_estimate": 1}}) + "\n",
        encoding="utf-8",
    )
    with pytest.raises(checker.EvidenceError, match="duplicate Criterion"):
        _report(evidence)


@pytest.mark.parametrize("value", ["-1", "+1", "01", "18446744073709551616"])
def test_noncanonical_or_out_of_range_allocation_count_is_rejected(
    evidence: tuple[Path, Path, str], value: str
) -> None:
    _, allocations, _ = evidence
    _write_allocations(allocations, first_value=value)
    with pytest.raises(checker.EvidenceError, match="canonical|exceeds u64"):
        _report(evidence)


def test_allocation_scope_and_order_are_exact(
    evidence: tuple[Path, Path, str]
) -> None:
    _, allocations, _ = evidence
    lines = allocations.read_text(encoding="utf-8").splitlines()
    lines[1] = "scope\tall-process-allocations"
    allocations.write_text("\n".join(lines) + "\n", encoding="utf-8")
    with pytest.raises(checker.EvidenceError, match="scope"):
        _report(evidence)

    _write_allocations(allocations)
    lines = allocations.read_text(encoding="utf-8").splitlines()
    lines[3], lines[4] = lines[4], lines[3]
    allocations.write_text("\n".join(lines) + "\n", encoding="utf-8")
    with pytest.raises(checker.EvidenceError, match="inventory mismatch"):
        _report(evidence)


def test_checkout_commit_must_match_trusted_expected_value(
    evidence: tuple[Path, Path, str]
) -> None:
    criterion, allocations, _ = evidence
    with pytest.raises(checker.EvidenceError, match="does not match checkout"):
        checker.build_report(
            criterion_dir=criterion,
            allocation_evidence=allocations,
            repository_root=REPOSITORY_ROOT,
            expected_source_commit="0" * 40,
        )
