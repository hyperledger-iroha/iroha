#!/usr/bin/env python3
"""Validate and seal SORA Parliament crypto benchmark evidence.

The gate consumes Criterion's exact ``benchmark.json.full_id`` inventory, the
benchmark binary's scoped allocation TSV, and a canonical allocation-ceiling
manifest. A separate candidate mode copies real measured allocation values into
the manifest shape; it never invents headroom or latency limits.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


REPORT_SCHEMA = "iroha.sora_parliament.crypto_benchmark_evidence.v1"
ALLOCATION_SCHEMA = "iroha.sora_parliament.crypto_allocations.v1"
ALLOCATION_BUDGET_SCHEMA = "iroha.sora_parliament.crypto_allocation_budgets.v1"
ALLOCATION_SCOPE = "measured-thread-logical-heap-requests-only"
CANONICAL_ALLOCATION_BUDGET_PATH = (
    "crates/iroha_crypto/benches/parliament_crypto_allocation_budgets.json"
)
INTEGRITY_DOMAIN = b"iroha.sora_parliament.crypto_benchmark_closure.v1\0"
FULL_GIT_SHA = re.compile(r"[0-9a-f]{40}")
CANONICAL_DECIMAL = re.compile(r"0|[1-9][0-9]*")

EXPECTED_BENCHMARK_IDS = (
    "parliament/threshold_bls/combine/threshold/4",
    "parliament/threshold_bls/combine/full/4",
    "parliament/threshold_bls/invalid_fast_fail/reordered/4",
    "parliament/threshold_bls/combine/threshold/16",
    "parliament/threshold_bls/combine/full/16",
    "parliament/threshold_bls/invalid_fast_fail/reordered/16",
    "parliament/threshold_bls/combine/threshold/31",
    "parliament/threshold_bls/combine/full/31",
    "parliament/threshold_bls/invalid_fast_fail/reordered/31",
    "parliament/threshold_bls/final_verify/0",
    "parliament/threshold_bls/final_verify/128",
    "parliament/threshold_bls/final_verify/16384",
    "parliament/timed_ovn/registration_roster_freeze/3",
    "parliament/timed_ovn/survivor_freeze/3",
    "parliament/timed_ovn/registration_roster_freeze/32",
    "parliament/timed_ovn/survivor_freeze/32",
    "parliament/timed_ovn/registration_roster_freeze/1000",
    "parliament/timed_ovn/survivor_freeze/1000",
    "parliament/timed_ovn/wire/registration_encode_3624_bytes",
    "parliament/timed_ovn/wire/registration_decode_verify_3624_bytes",
    "parliament/timed_ovn/wire/ballot_encode_2858_bytes",
    "parliament/timed_ovn/wire/ballot_decode_verify_2858_bytes",
    "parliament/timed_ovn/aggregate/proof_verified_3",
)

SOURCE_INPUT_PATHS = (
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain.toml",
    "crates/iroha_crypto/Cargo.toml",
    "crates/iroha_crypto/benches/parliament_crypto.rs",
    "crates/iroha_crypto/src/lib.rs",
    "crates/iroha_crypto/src/threshold_bls.rs",
    "crates/iroha_crypto/src/timed_ovn.rs",
    "crates/iroha_crypto/src/tle.rs",
    "scripts/check_sora_parliament_crypto_bench.py",
)

ALLOCATION_COLUMNS = (
    "allocation_calls",
    "reallocation_calls",
    "allocated_bytes",
    "reallocated_bytes",
)
ALLOCATION_BUDGET_COLUMNS = tuple(f"max_{column}" for column in ALLOCATION_COLUMNS)


class EvidenceError(RuntimeError):
    """Raised when benchmark evidence is incomplete or inconsistent."""


def _require_directory(path: Path, label: str) -> None:
    if path.is_symlink() or not path.is_dir():
        raise EvidenceError(f"{label} must be a regular, non-symlink directory: {path}")


def _require_regular_file(path: Path, root: Path, label: str) -> None:
    _require_directory(root, f"{label} root")
    try:
        relative = path.relative_to(root)
    except ValueError as error:
        raise EvidenceError(f"{label} escapes its declared root: {path}") from error
    current = root
    for component in relative.parts:
        current = current / component
        if current.is_symlink():
            raise EvidenceError(f"{label} contains a symlink: {current}")
    if not path.is_file():
        raise EvidenceError(f"{label} must be a regular file: {path}")


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _artifact_from_payload(payload: bytes, artifact_path: str) -> dict[str, Any]:
    return {
        "artifact_path": artifact_path,
        "sha256": _sha256(payload),
        "size_bytes": len(payload),
    }


def _artifact(path: Path, root: Path, artifact_path: str, label: str) -> dict[str, Any]:
    _require_regular_file(path, root, label)
    return _artifact_from_payload(path.read_bytes(), artifact_path)


def _reject_duplicate_json_keys(pairs: Sequence[tuple[str, object]]) -> dict[str, object]:
    document: dict[str, object] = {}
    for key, value in pairs:
        if key in document:
            raise EvidenceError(f"duplicate JSON key: {key}")
        document[key] = value
    return document


def _read_json(path: Path, root: Path, label: str) -> Any:
    _require_regular_file(path, root, label)
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            parse_float=Decimal,
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"invalid JSON in {label} {path}: {error}") from error


def _positive_finite_decimal(value: object, label: str) -> str:
    if isinstance(value, bool):
        raise EvidenceError(f"{label} must be a positive finite number")
    try:
        decimal = Decimal(value) if isinstance(value, int) else Decimal(str(value))
    except (InvalidOperation, TypeError, ValueError) as error:
        raise EvidenceError(f"{label} must be a positive finite number") from error
    if not decimal.is_finite() or decimal <= 0:
        raise EvidenceError(f"{label} must be a positive finite number")
    return str(decimal)


def _inventory_error(actual: Iterable[str]) -> EvidenceError:
    actual_set = set(actual)
    expected_set = set(EXPECTED_BENCHMARK_IDS)
    missing = sorted(expected_set - actual_set)
    extra = sorted(actual_set - expected_set)
    return EvidenceError(
        "Parliament benchmark inventory mismatch: "
        f"missing={missing or 'none'}, extra={extra or 'none'}"
    )


def read_criterion_samples(criterion_dir: Path) -> dict[str, dict[str, Any]]:
    """Read the exact current Criterion inventory by canonical ``full_id``."""

    _require_directory(criterion_dir, "Criterion root")
    samples: dict[str, dict[str, Any]] = {}
    for benchmark_path in sorted(criterion_dir.rglob("benchmark.json")):
        if benchmark_path.parent.name != "new":
            continue
        benchmark = _read_json(
            benchmark_path, criterion_dir, "Criterion benchmark metadata"
        )
        if not isinstance(benchmark, Mapping):
            raise EvidenceError(f"Criterion benchmark metadata is not an object: {benchmark_path}")
        full_id = benchmark.get("full_id")
        if not isinstance(full_id, str):
            raise EvidenceError(f"Criterion benchmark metadata lacks full_id: {benchmark_path}")
        if not full_id.startswith("parliament/"):
            continue
        if full_id in samples:
            raise EvidenceError(f"duplicate Criterion Parliament benchmark full_id: {full_id}")
        estimates_path = benchmark_path.with_name("estimates.json")
        estimates = _read_json(
            estimates_path, criterion_dir, "Criterion estimates"
        )
        if not isinstance(estimates, Mapping):
            raise EvidenceError(f"Criterion estimates are not an object: {estimates_path}")
        median = estimates.get("median")
        mean = estimates.get("mean")
        if not isinstance(median, Mapping) or "point_estimate" not in median:
            raise EvidenceError(
                f"Criterion estimates lack median.point_estimate: {estimates_path}"
            )
        if not isinstance(mean, Mapping) or "point_estimate" not in mean:
            raise EvidenceError(
                f"Criterion estimates lack mean.point_estimate: {estimates_path}"
            )
        sample_path = benchmark_path.with_name("sample.json")
        sample = _read_json(sample_path, criterion_dir, "Criterion sample")
        if not isinstance(sample, Mapping):
            raise EvidenceError(f"Criterion sample is not an object: {sample_path}")
        iterations = sample.get("iters")
        times = sample.get("times")
        if not isinstance(iterations, list) or not isinstance(times, list):
            raise EvidenceError(f"Criterion sample lacks iters/times arrays: {sample_path}")
        if len(iterations) != 10 or len(times) != 10:
            raise EvidenceError(
                f"Criterion sample must contain exactly 10 iterations and times: {sample_path}"
            )
        for index, value in enumerate(iterations):
            _positive_finite_decimal(value, f"Criterion iteration {index} for {full_id}")
        for index, value in enumerate(times):
            _positive_finite_decimal(value, f"Criterion time {index} for {full_id}")
        benchmark_relative = benchmark_path.relative_to(criterion_dir).as_posix()
        estimates_relative = estimates_path.relative_to(criterion_dir).as_posix()
        sample_relative = sample_path.relative_to(criterion_dir).as_posix()
        samples[full_id] = {
            "benchmark_metadata": _artifact(
                benchmark_path,
                criterion_dir,
                f"criterion/{benchmark_relative}",
                "Criterion benchmark metadata",
            ),
            "estimates": _artifact(
                estimates_path,
                criterion_dir,
                f"criterion/{estimates_relative}",
                "Criterion estimates",
            ),
            "sample": _artifact(
                sample_path,
                criterion_dir,
                f"criterion/{sample_relative}",
                "Criterion sample",
            ),
            "sample_count": 10,
            "mean_point_estimate_ns": _positive_finite_decimal(
                mean["point_estimate"],
                f"Criterion mean for {full_id}",
            ),
            "median_point_estimate_ns": _positive_finite_decimal(
                median["point_estimate"],
                f"Criterion median for {full_id}",
            ),
        }
    if set(samples) != set(EXPECTED_BENCHMARK_IDS):
        raise _inventory_error(samples)
    return {benchmark_id: samples[benchmark_id] for benchmark_id in EXPECTED_BENCHMARK_IDS}


def _canonical_nonnegative_integer(value: str, label: str) -> int:
    if not CANONICAL_DECIMAL.fullmatch(value):
        raise EvidenceError(f"{label} must be a canonical nonnegative integer")
    parsed = int(value)
    if parsed > (1 << 64) - 1:
        raise EvidenceError(f"{label} exceeds u64")
    return parsed


def read_allocation_evidence(path: Path) -> tuple[dict[str, dict[str, int]], bytes]:
    """Parse the deterministic current-thread allocation evidence TSV."""

    root = path.parent
    _require_regular_file(path, root, "allocation evidence")
    payload = path.read_bytes()
    if not payload.endswith(b"\n") or b"\r" in payload:
        raise EvidenceError("allocation evidence must use newline-terminated LF records")
    try:
        lines = payload.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise EvidenceError("allocation evidence must be UTF-8") from error
    expected_header = "benchmark_id\t" + "\t".join(ALLOCATION_COLUMNS)
    if len(lines) < 3:
        raise EvidenceError("allocation evidence is truncated")
    if lines[0] != f"schema\t{ALLOCATION_SCHEMA}":
        raise EvidenceError("unexpected allocation evidence schema")
    if lines[1] != f"scope\t{ALLOCATION_SCOPE}":
        raise EvidenceError("unexpected allocation evidence scope")
    if lines[2] != expected_header:
        raise EvidenceError("unexpected allocation evidence header")

    rows: dict[str, dict[str, int]] = {}
    ordered_ids: list[str] = []
    for line_number, line in enumerate(lines[3:], start=4):
        fields = line.split("\t")
        if len(fields) != 1 + len(ALLOCATION_COLUMNS):
            raise EvidenceError(f"allocation evidence row {line_number} has wrong field count")
        benchmark_id, *values = fields
        if benchmark_id in rows:
            raise EvidenceError(f"duplicate allocation benchmark identifier: {benchmark_id}")
        ordered_ids.append(benchmark_id)
        rows[benchmark_id] = {
            column: _canonical_nonnegative_integer(
                value, f"{benchmark_id} {column}"
            )
            for column, value in zip(ALLOCATION_COLUMNS, values, strict=True)
        }
    if tuple(ordered_ids) != EXPECTED_BENCHMARK_IDS:
        raise _inventory_error(ordered_ids)
    return rows, payload


def _canonical_json_bytes(document: Mapping[str, object]) -> bytes:
    return (json.dumps(document, indent=2, sort_keys=True) + "\n").encode("utf-8")


def build_budget_candidate(allocation_evidence: Path) -> dict[str, object]:
    """Copy actual measured values into the exact ceiling-manifest shape."""

    allocations, _ = read_allocation_evidence(allocation_evidence)
    return {
        "schema": ALLOCATION_BUDGET_SCHEMA,
        "scope": ALLOCATION_SCOPE,
        "benchmarks": [
            {
                "benchmark_id": benchmark_id,
                "ceilings": {
                    f"max_{column}": allocations[benchmark_id][column]
                    for column in ALLOCATION_COLUMNS
                },
            }
            for benchmark_id in EXPECTED_BENCHMARK_IDS
        ],
    }


def read_allocation_budget(path: Path) -> tuple[dict[str, dict[str, int]], bytes]:
    """Read a byte-canonical, exact-inventory allocation ceiling manifest."""

    root = path.parent
    _require_regular_file(path, root, "allocation budget manifest")
    payload = path.read_bytes()
    try:
        document = json.loads(
            payload.decode("utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"invalid allocation budget JSON: {error}") from error
    if not isinstance(document, Mapping):
        raise EvidenceError("allocation budget manifest must be an object")
    if set(document) != {"schema", "scope", "benchmarks"}:
        raise EvidenceError("allocation budget manifest has unexpected or missing fields")
    if document["schema"] != ALLOCATION_BUDGET_SCHEMA:
        raise EvidenceError("unexpected allocation budget schema")
    if document["scope"] != ALLOCATION_SCOPE:
        raise EvidenceError("unexpected allocation budget scope")
    benchmarks = document["benchmarks"]
    if not isinstance(benchmarks, list):
        raise EvidenceError("allocation budget benchmarks must be an ordered array")

    budgets: dict[str, dict[str, int]] = {}
    ordered_ids: list[str] = []
    for index, item in enumerate(benchmarks):
        if not isinstance(item, Mapping) or set(item) != {"benchmark_id", "ceilings"}:
            raise EvidenceError(f"allocation budget row {index} has unexpected fields")
        benchmark_id = item["benchmark_id"]
        if not isinstance(benchmark_id, str):
            raise EvidenceError(f"allocation budget row {index} lacks a string benchmark_id")
        if benchmark_id in budgets:
            raise EvidenceError(
                f"duplicate allocation budget benchmark identifier: {benchmark_id}"
            )
        ceilings = item["ceilings"]
        if not isinstance(ceilings, Mapping) or set(ceilings) != set(ALLOCATION_BUDGET_COLUMNS):
            raise EvidenceError(f"allocation budget row {benchmark_id} has unexpected ceilings")
        parsed: dict[str, int] = {}
        for column in ALLOCATION_BUDGET_COLUMNS:
            value = ceilings[column]
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                raise EvidenceError(f"{benchmark_id} {column} must be a nonnegative integer")
            if value > (1 << 64) - 1:
                raise EvidenceError(f"{benchmark_id} {column} exceeds u64")
            parsed[column] = value
        ordered_ids.append(benchmark_id)
        budgets[benchmark_id] = parsed
    if tuple(ordered_ids) != EXPECTED_BENCHMARK_IDS:
        raise _inventory_error(ordered_ids)
    if payload != _canonical_json_bytes(document):
        raise EvidenceError("allocation budget manifest is not canonical JSON")
    return budgets, payload


def evaluate_allocation_budgets(
    allocations: Mapping[str, Mapping[str, int]],
    budgets: Mapping[str, Mapping[str, int]],
) -> dict[str, dict[str, dict[str, int]]]:
    """Enforce every allocation metric and report its remaining headroom."""

    evaluation: dict[str, dict[str, dict[str, int]]] = {}
    for benchmark_id in EXPECTED_BENCHMARK_IDS:
        metrics: dict[str, dict[str, int]] = {}
        for column in ALLOCATION_COLUMNS:
            observed = allocations[benchmark_id][column]
            ceiling = budgets[benchmark_id][f"max_{column}"]
            if observed > ceiling:
                raise EvidenceError(
                    f"allocation budget exceeded for {benchmark_id} {column}: "
                    f"observed {observed}, ceiling {ceiling}"
                )
            metrics[column] = {
                "observed": observed,
                "ceiling": ceiling,
                "remaining": ceiling - observed,
            }
        evaluation[benchmark_id] = metrics
    return evaluation


def _run_git(repository_root: Path, arguments: Sequence[str]) -> bytes:
    """Run one read-only Git query and return its exact standard output."""

    _require_directory(repository_root, "repository root")
    try:
        result = subprocess.run(
            ["git", "-C", str(repository_root), *arguments],
            check=True,
            capture_output=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise EvidenceError(
            f"Git query failed ({' '.join(arguments)}): {error}"
        ) from error
    return result.stdout


def checkout_commit(repository_root: Path) -> str:
    """Return the exact checked-out commit, rejecting non-canonical output."""

    output = _run_git(repository_root, ("rev-parse", "--verify", "HEAD^{commit}"))
    try:
        commit = output.decode("ascii").strip()
    except UnicodeDecodeError as error:
        raise EvidenceError("repository HEAD is not ASCII") from error
    if not FULL_GIT_SHA.fullmatch(commit):
        raise EvidenceError("repository HEAD is not a lowercase full Git SHA")
    return commit


def require_clean_repository(repository_root: Path) -> None:
    """Reject tracked or untracked repository drift before sealing evidence."""

    status = _run_git(
        repository_root,
        ("status", "--porcelain=v1", "-z", "--untracked-files=all"),
    )
    if status:
        raise EvidenceError(
            "repository must be clean before Parliament benchmark evidence is sealed"
        )


def _require_committed_payload(
    repository_root: Path,
    source_commit: str,
    relative: str,
    payload: bytes,
) -> None:
    """Require one working-tree input to equal its blob at ``source_commit``."""

    try:
        committed = _run_git(
            repository_root,
            ("cat-file", "blob", f"{source_commit}:{relative}"),
        )
    except EvidenceError as error:
        raise EvidenceError(
            f"benchmark source input is not committed at {source_commit}: {relative}"
        ) from error
    if payload != committed:
        raise EvidenceError(
            f"benchmark source input does not match {source_commit}: {relative}"
        )


def read_source_inputs(
    repository_root: Path, source_commit: str
) -> list[dict[str, Any]]:
    """Hash fixed inputs only after matching each one to its committed blob."""

    inputs = []
    for relative in SOURCE_INPUT_PATHS:
        path = repository_root / relative
        _require_regular_file(path, repository_root, "benchmark source input")
        payload = path.read_bytes()
        _require_committed_payload(
            repository_root,
            source_commit,
            relative,
            payload,
        )
        inputs.append(_artifact_from_payload(payload, f"source/{relative}"))
    return inputs


def _integrity_digest(
    source_commit: str,
    source_inputs: Sequence[Mapping[str, object]],
    criterion_samples: Mapping[str, Mapping[str, object]],
    allocation_artifact: Mapping[str, object],
    allocation_budget_artifact: Mapping[str, object],
) -> str:
    artifacts: list[tuple[str, Mapping[str, object]]] = [
        ("allocation", allocation_artifact),
        ("allocation-budget", allocation_budget_artifact),
    ]
    artifacts.extend(("source", item) for item in source_inputs)
    for benchmark_id, sample in criterion_samples.items():
        artifacts.append((f"criterion:{benchmark_id}:benchmark", sample["benchmark_metadata"]))
        artifacts.append((f"criterion:{benchmark_id}:estimates", sample["estimates"]))
        artifacts.append((f"criterion:{benchmark_id}:sample", sample["sample"]))

    digest = hashlib.sha256()
    digest.update(INTEGRITY_DOMAIN)
    digest.update(source_commit.encode("ascii"))
    digest.update(b"\0")
    for role, artifact in sorted(
        artifacts, key=lambda item: (item[0], str(item[1]["artifact_path"]))
    ):
        digest.update(role.encode("utf-8"))
        digest.update(b"\0")
        digest.update(str(artifact["artifact_path"]).encode("utf-8"))
        digest.update(b"\0")
        digest.update(str(artifact["size_bytes"]).encode("ascii"))
        digest.update(b"\0")
        digest.update(str(artifact["sha256"]).encode("ascii"))
        digest.update(b"\0")
    return digest.hexdigest()


def build_report(
    *,
    criterion_dir: Path,
    allocation_evidence: Path,
    allocation_budget: Path,
    repository_root: Path,
    expected_source_commit: str,
) -> dict[str, Any]:
    """Build a canonical report after validating every source and sample."""

    if not FULL_GIT_SHA.fullmatch(expected_source_commit):
        raise EvidenceError("expected source commit must be a lowercase full Git SHA")
    source_commit = checkout_commit(repository_root)
    if source_commit != expected_source_commit:
        raise EvidenceError(
            "benchmark source commit does not match checkout: "
            f"expected {expected_source_commit}, found {source_commit}"
        )
    expected_budget_path = repository_root / CANONICAL_ALLOCATION_BUDGET_PATH
    if allocation_budget.absolute() != expected_budget_path.absolute():
        raise EvidenceError(
            "allocation budget must use the repository-owned canonical policy: "
            f"{CANONICAL_ALLOCATION_BUDGET_PATH}"
        )
    require_clean_repository(repository_root)
    source_inputs = read_source_inputs(repository_root, source_commit)
    criterion_samples = read_criterion_samples(criterion_dir)
    allocations, allocation_payload = read_allocation_evidence(allocation_evidence)
    budgets, budget_payload = read_allocation_budget(allocation_budget)
    _require_committed_payload(
        repository_root,
        source_commit,
        CANONICAL_ALLOCATION_BUDGET_PATH,
        budget_payload,
    )
    allocation_budget_evaluation = evaluate_allocation_budgets(allocations, budgets)
    allocation_artifact = {
        "artifact_path": "allocation/allocations.tsv",
        "sha256": _sha256(allocation_payload),
        "size_bytes": len(allocation_payload),
    }
    allocation_budget_artifact = {
        "artifact_path": "policy/allocation-budgets.json",
        "sha256": _sha256(budget_payload),
        "size_bytes": len(budget_payload),
    }
    integrity = _integrity_digest(
        source_commit,
        source_inputs,
        criterion_samples,
        allocation_artifact,
        allocation_budget_artifact,
    )
    require_clean_repository(repository_root)
    return {
        "schema": REPORT_SCHEMA,
        "source_commit": source_commit,
        "successful": True,
        "budget_policy": {
            "allocation_ceilings_enforced": True,
            "latency_ceilings_enforced": False,
        },
        "scope": {
            "allocation": ALLOCATION_SCOPE,
            "criterion_statistic": "median.point_estimate nanoseconds",
        },
        "inputs": {
            "source": source_inputs,
            "criterion": criterion_samples,
            "allocation": allocation_artifact,
            "allocation_budget": allocation_budget_artifact,
        },
        "measurements": {
            "allocations": allocations,
            "allocation_budget_evaluation": allocation_budget_evaluation,
            "criterion_mean_point_estimate_ns": {
                benchmark_id: sample["mean_point_estimate_ns"]
                for benchmark_id, sample in criterion_samples.items()
            },
            "criterion_median_point_estimate_ns": {
                benchmark_id: sample["median_point_estimate_ns"]
                for benchmark_id, sample in criterion_samples.items()
            },
        },
        "integrity": {
            "algorithm": "sha256",
            "domain": INTEGRITY_DOMAIN[:-1].decode("ascii"),
            "closure_sha256": integrity,
        },
    }


def canonical_report_bytes(report: Mapping[str, object]) -> bytes:
    """Render evidence with stable key ordering and whitespace."""

    return _canonical_json_bytes(report)


def write_canonical_document(
    path: Path, document: Mapping[str, object], label: str
) -> None:
    """Create one canonical JSON document without overwriting an existing path."""

    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open("xb") as output:
            output.write(_canonical_json_bytes(document))
    except OSError as error:
        raise EvidenceError(f"cannot create {label} {path}: {error}") from error


def write_report(path: Path, report: Mapping[str, object]) -> None:
    """Create one report without overwriting an existing path."""

    write_canonical_document(path, report, "benchmark report")


def verify_report(path: Path, expected: Mapping[str, object]) -> None:
    """Require byte-canonical equality with evidence rederived from raw inputs."""

    _require_regular_file(path, path.parent, "benchmark report")
    actual = path.read_bytes()
    canonical = canonical_report_bytes(expected)
    if actual != canonical:
        raise EvidenceError("benchmark report is non-canonical, stale, or tampered")


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--criterion-dir", type=Path)
    parser.add_argument("--allocation-evidence", type=Path, required=True)
    parser.add_argument("--allocation-budget", type=Path)
    parser.add_argument("--repository-root", type=Path, default=Path("."))
    parser.add_argument("--expected-source-commit")
    report_mode = parser.add_mutually_exclusive_group(required=True)
    report_mode.add_argument("--write-report", type=Path)
    report_mode.add_argument("--verify-report", type=Path)
    report_mode.add_argument("--write-budget-candidate", type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Run the fail-closed benchmark evidence validator."""

    args = _parser().parse_args(argv)
    try:
        if args.write_budget_candidate is not None:
            candidate = build_budget_candidate(args.allocation_evidence)
            write_canonical_document(
                args.write_budget_candidate,
                candidate,
                "allocation budget candidate",
            )
            print(
                "SORA Parliament allocation budget candidate written from "
                f"{len(EXPECTED_BENCHMARK_IDS)} measured cases: "
                f"{args.write_budget_candidate}"
            )
            return 0
        missing_arguments = [
            name
            for name, value in (
                ("--criterion-dir", args.criterion_dir),
                ("--allocation-budget", args.allocation_budget),
                ("--expected-source-commit", args.expected_source_commit),
            )
            if value is None
        ]
        if missing_arguments:
            raise EvidenceError(
                "report generation or verification requires "
                + ", ".join(missing_arguments)
            )
        report = build_report(
            criterion_dir=args.criterion_dir,
            allocation_evidence=args.allocation_evidence,
            allocation_budget=args.allocation_budget,
            repository_root=args.repository_root,
            expected_source_commit=args.expected_source_commit,
        )
        if args.write_report is not None:
            write_report(args.write_report, report)
            verify_report(args.write_report, report)
            report_path = args.write_report
        else:
            verify_report(args.verify_report, report)
            report_path = args.verify_report
    except EvidenceError as error:
        print(f"SORA Parliament crypto benchmark evidence rejected: {error}", file=sys.stderr)
        return 1
    print(
        "SORA Parliament crypto benchmark evidence accepted: "
        f"{len(EXPECTED_BENCHMARK_IDS)} cases, report={report_path}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
