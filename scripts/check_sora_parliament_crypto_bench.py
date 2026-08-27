#!/usr/bin/env python3
"""Validate and seal SORA Parliament crypto benchmark evidence.

The gate consumes Criterion's exact ``benchmark.json.full_id`` inventory plus
the benchmark binary's scoped allocation TSV. It intentionally records no
latency or allocation ceiling: V1 is an audit-evidence closure, and budgets must
be added only after measurements from a qualified runner exist.
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
ALLOCATION_SCOPE = "measured-thread-logical-heap-requests-only"
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


def _artifact(path: Path, root: Path, artifact_path: str, label: str) -> dict[str, Any]:
    _require_regular_file(path, root, label)
    payload = path.read_bytes()
    return {
        "artifact_path": artifact_path,
        "sha256": _sha256(payload),
        "size_bytes": len(payload),
    }


def _read_json(path: Path, root: Path, label: str) -> Any:
    _require_regular_file(path, root, label)
    try:
        return json.loads(path.read_text(encoding="utf-8"), parse_float=Decimal)
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
        if not isinstance(median, Mapping) or "point_estimate" not in median:
            raise EvidenceError(
                f"Criterion estimates lack median.point_estimate: {estimates_path}"
            )
        benchmark_relative = benchmark_path.relative_to(criterion_dir).as_posix()
        estimates_relative = estimates_path.relative_to(criterion_dir).as_posix()
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


def checkout_commit(repository_root: Path) -> str:
    """Return the exact checked-out commit, rejecting non-canonical output."""

    _require_directory(repository_root, "repository root")
    try:
        result = subprocess.run(
            ["git", "-C", str(repository_root), "rev-parse", "--verify", "HEAD^{commit}"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise EvidenceError(f"cannot resolve repository HEAD: {error}") from error
    commit = result.stdout.strip()
    if not FULL_GIT_SHA.fullmatch(commit):
        raise EvidenceError("repository HEAD is not a lowercase full Git SHA")
    return commit


def read_source_inputs(repository_root: Path) -> list[dict[str, Any]]:
    """Hash the fixed benchmark, implementation, dependency, and tool inputs."""

    inputs = []
    for relative in SOURCE_INPUT_PATHS:
        inputs.append(
            _artifact(
                repository_root / relative,
                repository_root,
                f"source/{relative}",
                "benchmark source input",
            )
        )
    return inputs


def _integrity_digest(
    source_commit: str,
    source_inputs: Sequence[Mapping[str, object]],
    criterion_samples: Mapping[str, Mapping[str, object]],
    allocation_artifact: Mapping[str, object],
) -> str:
    artifacts: list[tuple[str, Mapping[str, object]]] = [
        ("allocation", allocation_artifact)
    ]
    artifacts.extend(("source", item) for item in source_inputs)
    for benchmark_id, sample in criterion_samples.items():
        artifacts.append((f"criterion:{benchmark_id}:benchmark", sample["benchmark_metadata"]))
        artifacts.append((f"criterion:{benchmark_id}:estimates", sample["estimates"]))

    digest = hashlib.sha256()
    digest.update(INTEGRITY_DOMAIN)
    digest.update(source_commit.encode("ascii"))
    digest.update(b"\0")
    for role, artifact in sorted(artifacts, key=lambda item: (item[0], str(item[1]["artifact_path"]))):
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
    source_inputs = read_source_inputs(repository_root)
    criterion_samples = read_criterion_samples(criterion_dir)
    allocations, allocation_payload = read_allocation_evidence(allocation_evidence)
    allocation_artifact = {
        "artifact_path": "allocation/allocations.tsv",
        "sha256": _sha256(allocation_payload),
        "size_bytes": len(allocation_payload),
    }
    integrity = _integrity_digest(
        source_commit,
        source_inputs,
        criterion_samples,
        allocation_artifact,
    )
    return {
        "schema": REPORT_SCHEMA,
        "source_commit": source_commit,
        "successful": True,
        "budget_policy": {
            "allocation_ceilings_enforced": False,
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
        },
        "measurements": {
            "allocations": allocations,
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

    return (json.dumps(report, indent=2, sort_keys=True) + "\n").encode("utf-8")


def write_report(path: Path, report: Mapping[str, object]) -> None:
    """Create one report without overwriting or following an existing path."""

    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open("xb") as output:
            output.write(canonical_report_bytes(report))
    except OSError as error:
        raise EvidenceError(f"cannot create benchmark report {path}: {error}") from error


def verify_report(path: Path, expected: Mapping[str, object]) -> None:
    """Require byte-canonical equality with evidence rederived from raw inputs."""

    _require_regular_file(path, path.parent, "benchmark report")
    actual = path.read_bytes()
    canonical = canonical_report_bytes(expected)
    if actual != canonical:
        raise EvidenceError("benchmark report is non-canonical, stale, or tampered")


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--criterion-dir", type=Path, required=True)
    parser.add_argument("--allocation-evidence", type=Path, required=True)
    parser.add_argument("--repository-root", type=Path, default=Path("."))
    parser.add_argument("--expected-source-commit", required=True)
    report_mode = parser.add_mutually_exclusive_group(required=True)
    report_mode.add_argument("--write-report", type=Path)
    report_mode.add_argument("--verify-report", type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Run the fail-closed benchmark evidence validator."""

    args = _parser().parse_args(argv)
    try:
        report = build_report(
            criterion_dir=args.criterion_dir,
            allocation_evidence=args.allocation_evidence,
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
