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
CANONICAL_BUDGET_PATH = REPOSITORY_ROOT / checker.CANONICAL_ALLOCATION_BUDGET_PATH
SOURCE_COMMIT = "1" * 40


@pytest.fixture(autouse=True)
def committed_checkout(monkeypatch: pytest.MonkeyPatch) -> None:
    """Model a clean checkout whose fixed inputs equal their committed blobs."""

    def run_git(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
        if arguments == ("rev-parse", "--verify", "HEAD^{commit}"):
            return f"{SOURCE_COMMIT}\n".encode("ascii")
        if arguments == (
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ):
            return b""
        if arguments[:2] == ("cat-file", "blob"):
            commit, separator, relative = arguments[2].partition(":")
            assert commit == SOURCE_COMMIT and separator == ":"
            return (repository_root / relative).read_bytes()
        raise AssertionError(f"unexpected Git query: {arguments}")

    monkeypatch.setattr(checker, "_run_git", run_git)


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
                {
                    "mean": {"point_estimate": 1_001 + index},
                    "median": {"point_estimate": 1_000 + index},
                },
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        (sample / "sample.json").write_text(
            json.dumps(
                {
                    "sampling_mode": "Linear",
                    "iters": list(range(1, 11)),
                    "times": list(range(1_000, 1_010)),
                },
                sort_keys=True,
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
def evidence(tmp_path: Path) -> tuple[Path, Path, Path, str]:
    criterion = tmp_path / "criterion"
    allocations = tmp_path / "allocations.tsv"
    _write_criterion(criterion)
    _write_allocations(allocations)
    return (
        criterion,
        allocations,
        CANONICAL_BUDGET_PATH,
        checker.checkout_commit(REPOSITORY_ROOT),
    )


def _report(evidence: tuple[Path, Path, Path, str]) -> dict[str, object]:
    criterion, allocations, budget, commit = evidence
    return checker.build_report(
        criterion_dir=criterion,
        allocation_evidence=allocations,
        allocation_budget=budget,
        repository_root=REPOSITORY_ROOT,
        expected_source_commit=commit,
    )


def test_complete_inventory_produces_deterministic_hashed_report(
    evidence: tuple[Path, Path, Path, str],
) -> None:
    first = _report(evidence)
    second = _report(evidence)
    assert checker.canonical_report_bytes(first) == checker.canonical_report_bytes(second)
    assert first["schema"] == checker.REPORT_SCHEMA
    assert first["successful"] is True
    assert first["budget_policy"] == {
        "allocation_ceilings_enforced": True,
        "latency_ceilings_enforced": False,
    }
    assert len(first["inputs"]["source"]) == len(checker.SOURCE_INPUT_PATHS)
    assert set(first["measurements"]["allocations"]) == set(
        checker.EXPECTED_BENCHMARK_IDS
    )
    assert len(first["integrity"]["closure_sha256"]) == 64


def test_report_verification_rejects_raw_evidence_tampering(
    evidence: tuple[Path, Path, Path, str], tmp_path: Path
) -> None:
    report = _report(evidence)
    report_path = tmp_path / "report.json"
    checker.write_report(report_path, report)
    checker.verify_report(report_path, _report(evidence))

    criterion, _, _, _ = evidence
    estimates = criterion / "sanitized-case-00" / "new" / "estimates.json"
    estimates.write_text(
        json.dumps(
            {
                "mean": {"point_estimate": 9_998},
                "median": {"point_estimate": 9_999},
            }
        )
        + "\n",
        encoding="utf-8",
    )
    with pytest.raises(checker.EvidenceError, match="stale, or tampered"):
        checker.verify_report(report_path, _report(evidence))


def test_missing_or_extra_criterion_case_is_rejected(
    evidence: tuple[Path, Path, Path, str]
) -> None:
    criterion, _, _, _ = evidence
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
        json.dumps(
            {"mean": {"point_estimate": 1}, "median": {"point_estimate": 1}}
        )
        + "\n",
        encoding="utf-8",
    )
    (extra / "sample.json").write_text(
        json.dumps({"iters": [1] * 10, "times": [1] * 10}) + "\n",
        encoding="utf-8",
    )
    with pytest.raises(checker.EvidenceError, match="inventory mismatch"):
        _report(evidence)


def test_duplicate_criterion_identity_is_rejected(
    evidence: tuple[Path, Path, Path, str]
) -> None:
    criterion, _, _, _ = evidence
    duplicate = criterion / "duplicate" / "new"
    duplicate.mkdir(parents=True)
    (duplicate / "benchmark.json").write_text(
        json.dumps({"full_id": checker.EXPECTED_BENCHMARK_IDS[0]}) + "\n",
        encoding="utf-8",
    )
    (duplicate / "estimates.json").write_text(
        json.dumps(
            {"mean": {"point_estimate": 1}, "median": {"point_estimate": 1}}
        )
        + "\n",
        encoding="utf-8",
    )
    (duplicate / "sample.json").write_text(
        json.dumps({"iters": [1] * 10, "times": [1] * 10}) + "\n",
        encoding="utf-8",
    )
    with pytest.raises(checker.EvidenceError, match="duplicate Criterion"):
        _report(evidence)


def test_criterion_samples_are_exact_finite_and_duplicate_key_free(
    evidence: tuple[Path, Path, Path, str]
) -> None:
    criterion, _, _, _ = evidence
    sample = criterion / "sanitized-case-00" / "new" / "sample.json"
    sample.write_text(
        json.dumps({"iters": [1] * 9, "times": [1] * 9}) + "\n",
        encoding="utf-8",
    )
    with pytest.raises(checker.EvidenceError, match="exactly 10"):
        _report(evidence)

    benchmark_metadata = criterion / "sanitized-case-00" / "new" / "benchmark.json"
    sample.write_text(
        json.dumps({"iters": [1] * 10, "times": [1] * 10}) + "\n",
        encoding="utf-8",
    )
    benchmark_metadata.write_text(
        '{"full_id":"one","full_id":"parliament/threshold_bls/combine/threshold/4"}\n',
        encoding="utf-8",
    )
    with pytest.raises(checker.EvidenceError, match="duplicate JSON key"):
        _report(evidence)


@pytest.mark.parametrize("value", ["-1", "+1", "01", "18446744073709551616"])
def test_noncanonical_or_out_of_range_allocation_count_is_rejected(
    evidence: tuple[Path, Path, Path, str], value: str
) -> None:
    _, allocations, _, _ = evidence
    _write_allocations(allocations, first_value=value)
    with pytest.raises(checker.EvidenceError, match="canonical|exceeds u64"):
        _report(evidence)


def test_allocation_scope_and_order_are_exact(
    evidence: tuple[Path, Path, Path, str]
) -> None:
    _, allocations, _, _ = evidence
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


def test_allocation_budget_is_exact_inventory_and_enforced(
    evidence: tuple[Path, Path, Path, str]
) -> None:
    _, allocations, budget, _ = evidence
    parsed, _ = checker.read_allocation_budget(budget)
    assert tuple(parsed) == checker.EXPECTED_BENCHMARK_IDS
    assert parsed[checker.EXPECTED_BENCHMARK_IDS[0]]["max_allocation_calls"] == 14

    _write_allocations(allocations, first_value="15")
    with pytest.raises(checker.EvidenceError, match="allocation budget exceeded"):
        _report(evidence)


def test_allocation_budget_rejects_missing_duplicate_and_noncanonical_rows(
    evidence: tuple[Path, Path, Path, str], tmp_path: Path
) -> None:
    budget = tmp_path / "allocation-budget.json"
    document = json.loads(CANONICAL_BUDGET_PATH.read_text(encoding="utf-8"))
    document["benchmarks"].pop()
    budget.write_text(
        json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    with pytest.raises(checker.EvidenceError, match="inventory mismatch"):
        checker.read_allocation_budget(budget)

    candidate = checker.build_budget_candidate(evidence[1])
    candidate["benchmarks"][1] = candidate["benchmarks"][0]
    budget.write_text(
        json.dumps(candidate, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    with pytest.raises(checker.EvidenceError, match="duplicate allocation budget"):
        checker.read_allocation_budget(budget)

    candidate = checker.build_budget_candidate(evidence[1])
    budget.write_text(json.dumps(candidate), encoding="utf-8")
    with pytest.raises(checker.EvidenceError, match="not canonical JSON"):
        checker.read_allocation_budget(budget)


def test_report_rejects_substituted_allocation_policy(
    evidence: tuple[Path, Path, Path, str], tmp_path: Path
) -> None:
    criterion, allocations, _, commit = evidence
    substitute = tmp_path / "allocation-budget.json"
    substitute.write_bytes(CANONICAL_BUDGET_PATH.read_bytes())
    with pytest.raises(checker.EvidenceError, match="canonical policy"):
        checker.build_report(
            criterion_dir=criterion,
            allocation_evidence=allocations,
            allocation_budget=substitute,
            repository_root=REPOSITORY_ROOT,
            expected_source_commit=commit,
        )


def test_checkout_commit_must_match_trusted_expected_value(
    evidence: tuple[Path, Path, Path, str]
) -> None:
    criterion, allocations, budget, _ = evidence
    with pytest.raises(checker.EvidenceError, match="does not match checkout"):
        checker.build_report(
            criterion_dir=criterion,
            allocation_evidence=allocations,
            allocation_budget=budget,
            repository_root=REPOSITORY_ROOT,
            expected_source_commit="0" * 40,
        )


def test_dirty_tracked_source_is_rejected(
    evidence: tuple[Path, Path, Path, str], monkeypatch: pytest.MonkeyPatch
) -> None:
    run_git = checker._run_git

    def dirty_source(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
        if arguments and arguments[0] == "status":
            return b" M crates/iroha_crypto/src/threshold_bls.rs\0"
        return run_git(repository_root, arguments)

    monkeypatch.setattr(checker, "_run_git", dirty_source)
    with pytest.raises(checker.EvidenceError, match="repository must be clean"):
        _report(evidence)


def test_modified_committed_budget_is_rejected(
    evidence: tuple[Path, Path, Path, str], monkeypatch: pytest.MonkeyPatch
) -> None:
    run_git = checker._run_git

    def modified_budget(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
        if arguments == (
            "cat-file",
            "blob",
            f"{SOURCE_COMMIT}:{checker.CANONICAL_ALLOCATION_BUDGET_PATH}",
        ):
            return b"{}\n"
        return run_git(repository_root, arguments)

    monkeypatch.setattr(checker, "_run_git", modified_budget)
    with pytest.raises(checker.EvidenceError, match="does not match"):
        _report(evidence)


def test_uncommitted_canonical_budget_is_rejected(
    evidence: tuple[Path, Path, Path, str], monkeypatch: pytest.MonkeyPatch
) -> None:
    run_git = checker._run_git

    def missing_budget(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
        if arguments == (
            "cat-file",
            "blob",
            f"{SOURCE_COMMIT}:{checker.CANONICAL_ALLOCATION_BUDGET_PATH}",
        ):
            raise checker.EvidenceError("missing committed blob")
        return run_git(repository_root, arguments)

    monkeypatch.setattr(checker, "_run_git", missing_budget)
    with pytest.raises(checker.EvidenceError, match="is not committed"):
        _report(evidence)


def test_budget_candidate_mode_does_not_require_git(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    allocations = tmp_path / "allocations.tsv"
    _write_allocations(allocations)

    def reject_git(_repository_root: Path, _arguments: tuple[str, ...]) -> bytes:
        raise AssertionError("candidate mode must not query Git")

    monkeypatch.setattr(checker, "_run_git", reject_git)
    candidate = checker.build_budget_candidate(allocations)
    assert candidate["schema"] == checker.ALLOCATION_BUDGET_SCHEMA
    assert len(candidate["benchmarks"]) == len(checker.EXPECTED_BENCHMARK_IDS)
