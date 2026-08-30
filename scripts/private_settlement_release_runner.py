#!/usr/bin/env python3
"""Orchestrate real-process AtomicPrivateSettlementV1 release experiments.

The runner has two deliberately separate phases:

* ``plan`` freezes the exact N=2,3,4,8,16 matrix, harness executable,
  hardware description, configurations, canaries, and request identities.
  A plan is scaffolding and is never qualification evidence.
* ``execute`` checks out the same clean source revision, invokes the frozen
  external process harness once per request, validates every acknowledgement
  and measurement, and writes a publication fragment only after the complete
  matrix passes.

The external harness is the component that must start real Iroha processes,
exercise authenticated message control and persistence cuts, and capture the
network/storage surfaces.  This script never substitutes synthetic values for
missing harness output.  The harness protocol is a JSON request/response file
contract described by :func:`invoke_harness` and validated below.
"""

from __future__ import annotations

import argparse
import base64
import csv
import hashlib
import json
import math
import os
import re
import stat
import subprocess
import sys
import tempfile
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import private_settlement_benchmark_report as benchmark_report
import private_settlement_fault_report as fault_report
import private_settlement_leakage_audit as leakage_audit
import private_settlement_release_evidence as release_evidence

VERSION = 1
PROTOCOL = "AtomicPrivateSettlementV1"
PARTICIPANTS = (2, 3, 4, 8, 16)
PRIMARY_PARTICIPANTS = 3
VALIDATORS_PER_DATASPACE = 4
GLOBAL_VALIDATORS = 4
QUORUM = "3-of-4"
PROFILES = ("private", "transparent_control")
MIN_FAULT_SEEDS = 10
MAX_FAULT_SEEDS = 256
MAX_SEED = (1 << 64) - 1
MIN_WARMUPS = 5
MIN_MEASURED = 30
MAX_WARMUPS = 1_000
MAX_MEASURED = 1_000
MIN_BOOTSTRAP_ITERATIONS = 100
MAX_BOOTSTRAP_ITERATIONS = 10_000_000
MAX_OBSERVATION_COUNT = (1 << 64) - 1
DEFAULT_BOOTSTRAP_ITERATIONS = 2_000
MAX_HARNESS_RESPONSE_BYTES = 16 * 1024 * 1024
GIT_OBJECT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
SHA256 = re.compile(r"[0-9a-f]{64}")

SURFACE_FILES: Mapping[str, str] = {
    "block_wire_capture": "block-wire.bin",
    "event_capture": "events.json",
    "kura_artifact": "kura.bin",
    "merge_artifact": "merge.bin",
    "operator_log": "operator.json",
    "public_p2p_capture": "public-p2p.pcapng",
    "query_capture": "queries.json",
    "restricted_p2p_capture": "restricted-p2p.pcapng",
    "sanitized_capture": "sanitized-capture.pcapng",
    "snapshot_artifact": "snapshot.bin",
    "telemetry_capture": "telemetry.json",
    "torii_capture": "torii.pcapng",
}
REQUIRED_DIFFERENTIAL_STATE_CHANGES = {
    "block_wire_capture",
    "kura_artifact",
    "snapshot_artifact",
}

COMMON_RESPONSE_FIELDS = {
    "version",
    "protocol",
    "request_id",
    "invocation_nonce",
    "kind",
    "commit",
    "hardware_sha256",
    "hardware_profile_sha256",
    "configuration_sha256",
    "participants",
    "passed",
    "mandatory_signed_rs16_da_rbc",
    "signed_rs16_da_observations",
    "authenticated_message_control",
    "process_inventory",
    "payload",
}

# The reviewed partial harness now lives at
# ``scripts/private_settlement_real_process_harness.py`` and implements genuine
# N=2,3,4,8,16 private and transparent-control benchmark runs under this contract.
# TODO: Keep fault and leakage execution fail-closed until that harness gains
# reviewed real-process implementations for those branches.
HARNESS_CONTRACT: Mapping[str, Any] = {
    "version": VERSION,
    "invocation_arguments": [
        "--aps-request",
        "<request-json>",
        "--aps-response",
        "<response-json>",
        "--aps-evidence-dir",
        "<evidence-directory>",
    ],
    "request_transport": "json_file",
    "response_transport": "json_file",
    "response_required_on_success": True,
    "response_freshness": (
        "echo the per-invocation random 256-bit invocation_nonce"
    ),
    "invocation_nonce_scope": (
        "response freshness only; it must not alter the network workload or "
        "differential capture surfaces"
    ),
    "stdout_is_evidence": False,
    "undeclared_leakage_files_permitted": False,
}

FAULT_PAYLOAD_FIELDS = {
    "committee_validator_restarts",
    "maximum_simultaneously_unavailable_per_committee",
    "quorum_progress_with_one_unavailable",
    "coordinator_restarted",
    "global_node_restarted",
    "prepare_qc_normalization",
    "loss_trials",
    "phase_cut_partitions",
    "crash_recoveries",
    "atomicity",
    "all_nodes_converged",
}

BENCHMARK_CORRECTNESS_FIELDS = {
    "finalized_receipt_observed",
    "successful_leg_applications",
    "each_leg_applied_exactly_once",
    "partial_visible_observations",
    "partial_spendable_observations",
}


class RunnerError(ValueError):
    """Raised when a campaign cannot produce trustworthy release evidence."""


def exact_fields(value: Any, expected: set[str], label: str) -> dict[str, Any]:
    """Require an object with exactly the expected field set."""

    if not isinstance(value, dict):
        raise RunnerError(f"{label} must be an object")
    actual = set(value)
    if actual != expected:
        raise RunnerError(
            f"{label} fields mismatch; missing={sorted(expected - actual)} "
            f"unknown={sorted(actual - expected)}"
        )
    return value


def require_true(value: Any, label: str) -> None:
    """Require a literal JSON true value."""

    if value is not True:
        raise RunnerError(f"{label} must be true")


def nonnegative_integer(value: Any, label: str) -> int:
    """Return a non-negative, non-boolean integer."""

    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise RunnerError(f"{label} must be a non-negative integer")
    return value


def positive_integer(value: Any, label: str) -> int:
    """Return a positive, non-boolean integer."""

    result = nonnegative_integer(value, label)
    if result == 0:
        raise RunnerError(f"{label} must be positive")
    return result


def bounded_integer(value: Any, minimum: int, maximum: int, label: str) -> int:
    """Return an integer inside one inclusive, non-boolean range."""

    if isinstance(value, bool) or not isinstance(value, int):
        raise RunnerError(f"{label} must be an integer")
    if value < minimum or value > maximum:
        raise RunnerError(f"{label} must be in {minimum}..={maximum}")
    return value


def finite_nonnegative(value: Any, label: str) -> float:
    """Return one finite non-negative measurement."""

    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise RunnerError(f"{label} must be numeric")
    result = float(value)
    if not math.isfinite(result) or result < 0:
        raise RunnerError(f"{label} must be finite and non-negative")
    return result


def canonical_bytes(value: Any) -> bytes:
    """Encode stable compact JSON for request and commitment bindings."""

    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def object_digest(value: Any) -> str:
    """Return the SHA-256 of canonical compact JSON."""

    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def strict_json_loads(value: str, label: str) -> Any:
    """Decode strict JSON, rejecting duplicate keys and non-finite constants."""

    def object_from_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, item in pairs:
            if key in result:
                raise RunnerError(f"{label} contains duplicate key {key!r}")
            result[key] = item
        return result

    def reject_constant(constant: str) -> Any:
        raise RunnerError(f"{label} contains non-JSON constant {constant}")

    try:
        return json.loads(
            value,
            object_pairs_hook=object_from_pairs,
            parse_constant=reject_constant,
        )
    except json.JSONDecodeError as error:
        raise RunnerError(f"{label} is not valid JSON: {error}") from error


def write_json(path: Path, value: Any) -> None:
    """Write stable human-readable JSON with a terminal newline."""

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def write_jsonl(path: Path, values: Sequence[Mapping[str, Any]]) -> None:
    """Write canonical JSON Lines without accepting an empty collection."""

    if not values:
        raise RunnerError(f"refusing to write empty JSONL evidence: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(
            json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
            + "\n"
            for value in values
        ),
        encoding="utf-8",
    )


def _open_regular_nofollow(path: Path) -> tuple[int, os.stat_result]:
    """Open one final path component without following a symbolic link."""

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise RunnerError(
            f"evidence path must be a readable regular non-symlink file: {path}"
        ) from error
    metadata = os.fstat(descriptor)
    if not stat.S_ISREG(metadata.st_mode):
        os.close(descriptor)
        raise RunnerError(f"evidence path must be a regular file: {path}")
    return descriptor, metadata


def file_binding(path: Path, *, relative_to: Path | None = None) -> dict[str, Any]:
    """Hash one regular non-symlink file and optionally bind its relative path."""

    descriptor, metadata = _open_regular_nofollow(path)
    digest = hashlib.sha256()
    bytes_read = 0
    with os.fdopen(descriptor, "rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
            bytes_read += len(chunk)
        final_metadata = os.fstat(stream.fileno())
    stable_fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if bytes_read != metadata.st_size or any(
        getattr(metadata, field) != getattr(final_metadata, field)
        for field in stable_fields
    ):
        raise RunnerError(f"evidence file changed while it was hashed: {path}")
    result: dict[str, Any] = {
        "sha256": digest.hexdigest(),
        "bytes": bytes_read,
    }
    if relative_to is not None:
        try:
            relative = path.relative_to(relative_to)
        except ValueError as error:
            raise RunnerError(f"{path} is outside {relative_to}") from error
        result["path"] = relative.as_posix()
    return result


def safe_relative_path(value: Any, label: str) -> PurePosixPath:
    """Parse a normalized relative POSIX path."""

    if not isinstance(value, str) or not value:
        raise RunnerError(f"{label} must be a non-empty relative path")
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or path.as_posix() != value
        or any(part in ("", ".", "..") for part in path.parts)
    ):
        raise RunnerError(f"{label} must be a normalized relative POSIX path")
    return path


def regular_file_under(root: Path, relative: PurePosixPath, label: str) -> Path:
    """Resolve a regular file below ``root`` without accepting symlink components."""

    try:
        canonical_root = root.resolve(strict=True)
    except OSError as error:
        raise RunnerError(f"{label} root cannot be resolved: {error}") from error
    if root.is_symlink() or not canonical_root.is_dir():
        raise RunnerError(f"{label} root must be a regular directory")
    candidate = canonical_root
    for part in relative.parts:
        candidate /= part
        if candidate.is_symlink():
            raise RunnerError(f"{label} traverses a symbolic link")
    try:
        resolved = candidate.resolve(strict=True)
    except OSError as error:
        raise RunnerError(f"{label} cannot be resolved: {error}") from error
    if not resolved.is_relative_to(canonical_root) or not resolved.is_file():
        raise RunnerError(f"{label} must be a regular file beneath its root")
    return candidate


def read_json_file(path: Path, label: str) -> Any:
    """Read one non-symlink regular file as strict UTF-8 JSON."""

    descriptor, _ = _open_regular_nofollow(path)
    try:
        with os.fdopen(descriptor, "rb") as stream:
            raw = stream.read(MAX_HARNESS_RESPONSE_BYTES + 1)
    except OSError as error:
        raise RunnerError(f"cannot read {label}: {error}") from error
    if len(raw) > MAX_HARNESS_RESPONSE_BYTES:
        raise RunnerError(f"{label} exceeds the bounded JSON size")
    try:
        text = raw.decode("utf-8")
    except UnicodeError as error:
        raise RunnerError(f"{label} is not UTF-8: {error}") from error
    return strict_json_loads(text, label)


def read_bound_json_file(
    path: Path, expected: Mapping[str, Any], label: str
) -> Any:
    """Read strict JSON whose content must retain an exact frozen binding."""

    before = file_binding(path)
    if before != dict(expected):
        raise RunnerError(f"{label} binding differs from the frozen plan")
    value = read_json_file(path, label)
    if file_binding(path) != before:
        raise RunnerError(f"{label} changed while it was read")
    return value


def minimum_signed_rs16_da_observations(participants: int) -> int:
    """Require at least one signed DA observation from every validator process."""

    if participants not in PARTICIPANTS:
        raise RunnerError("unsupported participant count for DA observation policy")
    return (participants + 1) * VALIDATORS_PER_DATASPACE


def build_canary_manifest(commit: str) -> dict[str, Any]:
    """Build two deterministic secret sets for the primary differential run."""

    if GIT_OBJECT.fullmatch(commit) is None:
        raise RunnerError("commit must be a full lowercase Git object id")
    seed = hashlib.sha256(f"{PROTOCOL}:{commit}:leakage-canaries".encode()).digest()
    tag_a = seed[:12].hex()
    tag_b = seed[12:24].hex()
    amount_a = int.from_bytes(seed[:16], "big") | (1 << 127)
    amount_b = int.from_bytes(hashlib.sha256(seed).digest()[:16], "big") | (1 << 127)
    entries = {
        "account_id": ("text", f"aps-account-{tag_a}@canary.invalid"),
        "account_id_variant_b": ("text", f"aps-account-{tag_b}@canary.invalid"),
        "amount": ("integer", amount_a),
        "amount_variant_b": ("integer", amount_b),
        "asset_alias": ("text", f"aps-cbdc-alias-{tag_a}"),
        "asset_alias_variant_b": ("text", f"aps-cbdc-alias-{tag_b}"),
        "asset_id": ("text", f"aps-cbdc-{tag_a}#canary.invalid"),
        "asset_id_variant_b": ("text", f"aps-cbdc-{tag_b}#canary.invalid"),
        "capsule": ("binary_base64", base64.b64encode(seed + seed[:16]).decode()),
        "capsule_variant_b": (
            "binary_base64",
            base64.b64encode(hashlib.sha512(seed).digest()[:48]).decode(),
        ),
        "memo": ("text", f"aps-private-memo-{tag_a}-never-public"),
        "memo_variant_b": ("text", f"aps-private-memo-{tag_b}-never-public"),
    }
    return {
        "version": VERSION,
        "canaries": [
            {"name": name, "kind": kind, "value": value}
            for name, (kind, value) in sorted(entries.items())
        ],
    }


def canaries_for_variant(
    manifest: Mapping[str, Any], variant: str
) -> list[dict[str, Any]]:
    """Select the six planted values for one side of the differential pair."""

    if variant not in ("left", "right"):
        raise RunnerError("leakage variant must be left or right")
    suffix = "" if variant == "left" else "_variant_b"
    by_name = {entry["name"]: entry for entry in manifest["canaries"]}
    selected = []
    for base in release_evidence.REQUIRED_LEAKAGE_CANARY_NAMES:
        name = f"{base}{suffix}"
        if name not in by_name:
            raise RunnerError(f"canary manifest lacks {name}")
        selected.append(dict(by_name[name]))
    return selected


def build_configuration(
    participants: int,
    *,
    seeds: Sequence[int],
    warmups: int,
    measured: int,
) -> dict[str, Any]:
    """Build the exact high-level configuration consumed by the process harness."""

    if participants not in PARTICIPANTS:
        raise RunnerError("unsupported real-process participant count")
    normalized_seeds = verify_seed_policy(seeds)
    bounded_integer(warmups, MIN_WARMUPS, MAX_WARMUPS, "warmups")
    bounded_integer(measured, MIN_MEASURED, MAX_MEASURED, "measured runs")
    return {
        "version": VERSION,
        "protocol": PROTOCOL,
        "participants": participants,
        "primary_paper_configuration": participants == PRIMARY_PARTICIPANTS,
        "topology": {
            "global_validators": GLOBAL_VALIDATORS,
            "participant_dataspaces": list(range(participants)),
            "validators_per_dataspace": VALIDATORS_PER_DATASPACE,
            "total_validator_processes": (participants + 1)
            * VALIDATORS_PER_DATASPACE,
            "coordinator_processes": 1,
            "quorum": QUORUM,
        },
        "consensus": {
            "mandatory_signed_rs16_da_rbc": True,
            "minimum_signed_rs16_da_observations_per_run": (
                minimum_signed_rs16_da_observations(participants)
            ),
            "authenticated_message_control": True,
            "maximum_simultaneously_unavailable_per_committee": 1,
            "legacy_rbc_bypass_permitted": False,
        },
        "fault_matrix": {
            "seeds": list(normalized_seeds),
            "loss_phases": list(fault_report.REQUIRED_LOSS_PHASES),
            "loss_percentages": list(fault_report.REQUIRED_LOSS_PERCENTAGES),
            "phase_cuts": list(fault_report.REQUIRED_PHASE_CUTS),
            "crash_boundaries": list(fault_report.REQUIRED_CRASH_BOUNDARIES),
            "restart_one_validator_in_each_dataspace": True,
            "restart_coordinator": True,
            "restart_global_node": True,
            "continuous_atomicity_checks": True,
            "prepare_qc_normalization": {
                "first_signer_subset": [0, 1, 2],
                "second_signer_subset": [0, 1, 3],
                "accept_equivalent_subsets_only_for_identical_body": True,
                "bind_authority_indices": True,
                "bind_every_signed_body": True,
                "reject_changed_certified_body": True,
            },
        },
        "benchmark": {
            "profiles": list(PROFILES),
            "warmups_per_profile": warmups,
            "measured_bundles_per_profile": measured,
            "seeds": list(normalized_seeds),
            "serial_execution": True,
            "stages_private": list(benchmark_report.REQUIRED_PRIVATE_STAGES),
            "stages_transparent_control": ["global_finality", "end_to_end"],
            "resources": list(benchmark_report.RESOURCE_FIELDS),
        },
        "leakage": {
            "enabled": participants == PRIMARY_PARTICIPANTS,
            "differential_variants": ["left", "right"],
            "capture_surfaces": sorted(SURFACE_FILES),
            "message_count_channels": list(leakage_audit.REQUIRED_COUNT_CHANNELS),
            "only_secret_fields_change": True,
        },
    }


def job_with_id(value: Mapping[str, Any]) -> dict[str, Any]:
    """Attach a deterministic request id to a job description."""

    body = dict(value)
    request_id = object_digest(body)
    return {"request_id": request_id, **body}


def build_jobs(
    configuration_sha256: Mapping[int, str],
    seeds: Sequence[int],
    warmups: int,
    measured: int,
    canary_manifest: Mapping[str, Any],
) -> list[dict[str, Any]]:
    """Build the canonical full release job matrix."""

    jobs: list[dict[str, Any]] = []
    for participants in PARTICIPANTS:
        for run, seed in enumerate(seeds):
            jobs.append(
                job_with_id(
                    {
                        "kind": "fault",
                        "participants": participants,
                        "seed": seed,
                        "run": run,
                        "configuration_sha256": configuration_sha256[participants],
                    }
                )
            )
    for profile in PROFILES:
        for participants in PARTICIPANTS:
            for run in range(warmups):
                jobs.append(
                    job_with_id(
                        {
                            "kind": "benchmark",
                            "profile": profile,
                            "participants": participants,
                            "seed": seeds[run % len(seeds)],
                            "run": run,
                            "warmup": True,
                            "configuration_sha256": configuration_sha256[
                                participants
                            ],
                        }
                    )
                )
            for run in range(measured):
                jobs.append(
                    job_with_id(
                        {
                            "kind": "benchmark",
                            "profile": profile,
                            "participants": participants,
                            "seed": seeds[run % len(seeds)],
                            "run": run,
                            "warmup": False,
                            "configuration_sha256": configuration_sha256[
                                participants
                            ],
                        }
                    )
                )
    for variant in ("left", "right"):
        injected = canaries_for_variant(canary_manifest, variant)
        jobs.append(
            job_with_id(
                {
                    "kind": "leakage",
                    "participants": PRIMARY_PARTICIPANTS,
                    "seed": seeds[0],
                    "run": 0,
                    "variant": variant,
                    "canary_names": [entry["name"] for entry in injected],
                    "canary_commitments": {
                        entry["name"]: object_digest(entry) for entry in injected
                    },
                    "configuration_sha256": configuration_sha256[
                        PRIMARY_PARTICIPANTS
                    ],
                }
            )
        )
    if len({job["request_id"] for job in jobs}) != len(jobs):
        raise RunnerError("job matrix contains a duplicate request identity")
    return jobs


def verify_seed_policy(seeds: Sequence[int]) -> tuple[int, ...]:
    """Require a canonical set of at least ten randomized campaign seeds."""

    if len(seeds) < MIN_FAULT_SEEDS:
        raise RunnerError(f"at least {MIN_FAULT_SEEDS} seeds are required")
    if len(seeds) > MAX_FAULT_SEEDS:
        raise RunnerError(f"at most {MAX_FAULT_SEEDS} seeds are permitted")
    if any(
        isinstance(seed, bool)
        or not isinstance(seed, int)
        or seed < 0
        or seed > MAX_SEED
        for seed in seeds
    ):
        raise RunnerError("seeds must be unsigned 64-bit integers")
    if list(seeds) != sorted(set(seeds)):
        raise RunnerError("seeds must be unique and sorted")
    return tuple(seeds)


def verify_source_checkout(source_root: Path, commit: str) -> None:
    """Require the exact clean release checkout before planning or execution."""

    try:
        head = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=source_root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        dirty = subprocess.run(
            ["git", "status", "--porcelain=v1", "--untracked-files=all"],
            cwd=source_root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        git_dir = Path(
            subprocess.run(
                ["git", "rev-parse", "--absolute-git-dir"],
                cwd=source_root,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
        )
        git_paths = {
            name: git_dir / name
            for name in (
                "MERGE_HEAD",
                "CHERRY_PICK_HEAD",
                "REVERT_HEAD",
                "rebase-merge",
                "rebase-apply",
            )
        }
    except (OSError, subprocess.CalledProcessError) as error:
        raise RunnerError(f"cannot authenticate source checkout: {error}") from error
    if head != commit:
        raise RunnerError(f"source checkout is {head}, expected {commit}")
    if dirty:
        raise RunnerError("source checkout must be completely clean")
    active_operations = sorted(
        name for name, candidate in git_paths.items() if candidate.exists()
    )
    if active_operations:
        raise RunnerError(
            f"source checkout has active Git operations: {active_operations}"
        )


def verify_harness(path: Path) -> dict[str, Any]:
    """Authenticate one executable, regular, non-symlink harness file."""

    if path.is_symlink() or not path.is_file() or not os.access(path, os.X_OK):
        raise RunnerError("harness must be an executable regular non-symlink file")
    return file_binding(path)


def require_external_output(output_dir: Path, source_root: Path) -> None:
    """Keep generated evidence outside the clean release checkout."""

    source = source_root.resolve(strict=True)
    candidate = output_dir.resolve(strict=False)
    if candidate == source or candidate.is_relative_to(source):
        raise RunnerError(
            "plan and evidence outputs must live outside the clean source checkout"
        )


def validate_benchmark_baseline(value: Any, label: str) -> dict[str, Any]:
    """Require a passing strict benchmark report before using it as a baseline."""

    record = exact_fields(
        value,
        {
            "version",
            "protocol",
            "commit",
            "environment",
            "requirements",
            "profiles",
            "regressions",
            "passed",
        },
        label,
    )
    if (
        record["version"] != VERSION
        or record["protocol"] != PROTOCOL
        or GIT_OBJECT.fullmatch(record["commit"] or "") is None
        or record["regressions"] != []
        or record["passed"] is not True
    ):
        raise RunnerError(f"{label} must be a passing V1 benchmark report")
    return record


def create_plan(
    output_dir: Path,
    *,
    source_root: Path,
    commit: str,
    harness: Path,
    hardware_description: Path,
    seeds: Sequence[int],
    warmups: int,
    measured: int,
    bootstrap_iterations: int,
    benchmark_baseline: Path | None,
) -> Path:
    """Freeze a complete deterministic campaign plan atomically."""

    if output_dir.exists():
        raise RunnerError(f"plan output already exists: {output_dir}")
    require_external_output(output_dir, source_root)
    if GIT_OBJECT.fullmatch(commit) is None:
        raise RunnerError("commit must be a full lowercase Git object id")
    normalized_seeds = verify_seed_policy(seeds)
    bounded_integer(warmups, MIN_WARMUPS, MAX_WARMUPS, "warmups")
    bounded_integer(measured, MIN_MEASURED, MAX_MEASURED, "measured runs")
    bounded_integer(
        bootstrap_iterations,
        MIN_BOOTSTRAP_ITERATIONS,
        MAX_BOOTSTRAP_ITERATIONS,
        "bootstrap iterations",
    )
    verify_source_checkout(source_root, commit)
    harness_binding = verify_harness(harness)
    hardware_profile_sha256 = release_evidence._validate_hardware_description(
        hardware_description, commit=commit
    )
    if hardware_description.is_symlink() or not hardware_description.is_file():
        raise RunnerError("hardware description must be a regular non-symlink file")
    hardware_binding = file_binding(hardware_description)
    baseline_binding = None
    if benchmark_baseline is not None:
        if benchmark_baseline.is_symlink() or not benchmark_baseline.is_file():
            raise RunnerError("benchmark baseline must be a regular non-symlink file")
        document = read_json_file(benchmark_baseline, "benchmark baseline")
        validate_benchmark_baseline(document, "benchmark baseline")
        baseline_binding = file_binding(benchmark_baseline)

    parent = output_dir.parent.resolve()
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="aps-plan-", dir=parent) as temporary:
        root = Path(temporary)
        hardware_target = root / "hardware-description-v1.json"
        copy_bound_file(
            hardware_description, hardware_target, expected=hardware_binding
        )
        release_evidence._validate_hardware_description(hardware_target, commit=commit)
        baseline_reference = None
        if benchmark_baseline is not None and baseline_binding is not None:
            baseline_target = root / "benchmark-baseline-v1.json"
            copy_bound_file(
                benchmark_baseline, baseline_target, expected=baseline_binding
            )
            validate_benchmark_baseline(
                read_json_file(baseline_target, "benchmark baseline"),
                "benchmark baseline",
            )
            baseline_reference = {
                "path": baseline_target.relative_to(root).as_posix(),
                **baseline_binding,
            }
        canary_manifest = build_canary_manifest(commit)
        canary_target = root / "canary-manifest-v1.json"
        write_json(canary_target, canary_manifest)
        canary_binding = file_binding(canary_target)

        configurations = []
        configuration_digests: dict[int, str] = {}
        for participants in PARTICIPANTS:
            path = root / "configurations" / f"n{participants}.json"
            write_json(
                path,
                build_configuration(
                    participants,
                    seeds=normalized_seeds,
                    warmups=warmups,
                    measured=measured,
                ),
            )
            binding = file_binding(path, relative_to=root)
            configuration_digests[participants] = binding["sha256"]
            configurations.append(
                {
                    "participants": participants,
                    "validators_per_dataspace": VALIDATORS_PER_DATASPACE,
                    "quorum": QUORUM,
                    "mandatory_signed_rs16_da_rbc": True,
                    **binding,
                }
            )
        configuration_manifest = {
            "version": VERSION,
            "protocol": PROTOCOL,
            "commit": commit,
            "configurations": configurations,
            "passed": True,
        }
        configuration_manifest_path = root / "configuration-manifest-v1.json"
        write_json(configuration_manifest_path, configuration_manifest)

        jobs = build_jobs(
            configuration_digests,
            normalized_seeds,
            warmups,
            measured,
            canary_manifest,
        )
        plan = {
            "version": VERSION,
            "protocol": PROTOCOL,
            "commit": commit,
            "worktree_clean": True,
            "publication_evidence": False,
            "execution_required": True,
            "harness": {
                "sha256": harness_binding["sha256"],
                "bytes": harness_binding["bytes"],
            },
            "harness_contract": dict(HARNESS_CONTRACT),
            "benchmark_baseline": baseline_reference,
            "hardware": {
                "path": hardware_target.relative_to(root).as_posix(),
                "profile_sha256": hardware_profile_sha256,
                **file_binding(hardware_target),
            },
            "canary_manifest": {
                "path": canary_target.relative_to(root).as_posix(),
                **canary_binding,
            },
            "configuration_manifest": {
                "path": configuration_manifest_path.relative_to(root).as_posix(),
                **file_binding(configuration_manifest_path),
            },
            "requirements": {
                "participants": list(PARTICIPANTS),
                "primary_participants": PRIMARY_PARTICIPANTS,
                "validators_per_dataspace": VALIDATORS_PER_DATASPACE,
                "quorum": QUORUM,
                "seeds": list(normalized_seeds),
                "warmups": warmups,
                "measured": measured,
                "bootstrap_iterations": bootstrap_iterations,
                "loss_phases": list(fault_report.REQUIRED_LOSS_PHASES),
                "loss_percentages": list(fault_report.REQUIRED_LOSS_PERCENTAGES),
                "phase_cuts": list(fault_report.REQUIRED_PHASE_CUTS),
                "crash_boundaries": list(fault_report.REQUIRED_CRASH_BOUNDARIES),
                "capture_surfaces": sorted(SURFACE_FILES),
                "message_count_channels": list(
                    leakage_audit.REQUIRED_COUNT_CHANNELS
                ),
            },
            "jobs": jobs,
        }
        write_json(root / "run-plan-v1.json", plan)
        if verify_harness(harness) != harness_binding:
            raise RunnerError("harness executable changed while the plan was frozen")
        verify_source_checkout(source_root, commit)
        root.rename(output_dir)
    return output_dir / "run-plan-v1.json"


def load_plan(path: Path) -> tuple[dict[str, Any], Path]:
    """Load and authenticate a frozen plan and every bound input file."""

    if path.is_symlink() or not path.is_file():
        raise RunnerError("plan must be a regular non-symlink file")
    root = path.parent.resolve(strict=True)
    initial_plan_binding = file_binding(path)
    plan = read_json_file(path, "plan")
    if file_binding(path) != initial_plan_binding:
        raise RunnerError("plan changed while it was read")
    expected = {
        "version",
        "protocol",
        "commit",
        "worktree_clean",
        "publication_evidence",
        "execution_required",
        "harness",
        "harness_contract",
        "benchmark_baseline",
        "hardware",
        "canary_manifest",
        "configuration_manifest",
        "requirements",
        "jobs",
    }
    exact_fields(plan, expected, "plan")
    if (
        plan["version"] != VERSION
        or plan["protocol"] != PROTOCOL
        or GIT_OBJECT.fullmatch(plan["commit"] or "") is None
        or plan["worktree_clean"] is not True
        or plan["publication_evidence"] is not False
        or plan["execution_required"] is not True
    ):
        raise RunnerError("plan header is invalid")
    harness_record = exact_fields(
        plan["harness"], {"sha256", "bytes"}, "plan.harness"
    )
    if (
        not isinstance(harness_record["sha256"], str)
        or SHA256.fullmatch(harness_record["sha256"]) is None
        or harness_record["sha256"] == "0" * 64
    ):
        raise RunnerError("plan.harness.sha256 is invalid")
    positive_integer(harness_record["bytes"], "plan.harness.bytes")
    if plan["harness_contract"] != HARNESS_CONTRACT:
        raise RunnerError("plan uses an unsupported harness contract")
    requirements = exact_fields(
        plan["requirements"],
        {
            "participants",
            "primary_participants",
            "validators_per_dataspace",
            "quorum",
            "seeds",
            "warmups",
            "measured",
            "bootstrap_iterations",
            "loss_phases",
            "loss_percentages",
            "phase_cuts",
            "crash_boundaries",
            "capture_surfaces",
            "message_count_channels",
        },
        "plan.requirements",
    )
    if (
        requirements["participants"] != list(PARTICIPANTS)
        or requirements["primary_participants"] != PRIMARY_PARTICIPANTS
        or requirements["validators_per_dataspace"] != VALIDATORS_PER_DATASPACE
        or requirements["quorum"] != QUORUM
        or requirements["loss_phases"] != list(fault_report.REQUIRED_LOSS_PHASES)
        or requirements["loss_percentages"]
        != list(fault_report.REQUIRED_LOSS_PERCENTAGES)
        or requirements["phase_cuts"] != list(fault_report.REQUIRED_PHASE_CUTS)
        or requirements["crash_boundaries"]
        != list(fault_report.REQUIRED_CRASH_BOUNDARIES)
        or requirements["capture_surfaces"] != sorted(SURFACE_FILES)
        or requirements["message_count_channels"]
        != list(leakage_audit.REQUIRED_COUNT_CHANNELS)
    ):
        raise RunnerError("plan requirement matrix is not canonical")
    seeds = verify_seed_policy(requirements["seeds"])
    bounded_integer(
        requirements["warmups"], MIN_WARMUPS, MAX_WARMUPS, "plan warmups"
    )
    bounded_integer(
        requirements["measured"],
        MIN_MEASURED,
        MAX_MEASURED,
        "plan measured runs",
    )
    bounded_integer(
        requirements["bootstrap_iterations"],
        MIN_BOOTSTRAP_ITERATIONS,
        MAX_BOOTSTRAP_ITERATIONS,
        "plan bootstrap iterations",
    )

    def check_bound_file(
        reference: Any, label: str, *, extra_fields: set[str] | None = None
    ) -> Path:
        expected_fields = {"path", "sha256", "bytes"}
        if extra_fields is not None:
            expected_fields.update(extra_fields)
        record = exact_fields(reference, expected_fields, label)
        relative = safe_relative_path(record["path"], f"{label}.path")
        candidate = regular_file_under(root, relative, label)
        actual = file_binding(candidate)
        if actual != {"sha256": record["sha256"], "bytes": record["bytes"]}:
            raise RunnerError(f"{label} binding differs from the frozen plan")
        return candidate

    hardware = check_bound_file(
        plan["hardware"], "plan.hardware", extra_fields={"profile_sha256"}
    )
    canary_path = check_bound_file(plan["canary_manifest"], "plan.canary_manifest")
    configuration_manifest_path = check_bound_file(
        plan["configuration_manifest"], "plan.configuration_manifest"
    )
    if plan["benchmark_baseline"] is not None:
        baseline_path = check_bound_file(
            plan["benchmark_baseline"], "plan.benchmark_baseline"
        )
        baseline_document = read_bound_json_file(
            baseline_path,
            {
                "sha256": plan["benchmark_baseline"]["sha256"],
                "bytes": plan["benchmark_baseline"]["bytes"],
            },
            "benchmark baseline",
        )
        validate_benchmark_baseline(baseline_document, "benchmark baseline")
    hardware_profile_sha256 = release_evidence._validate_hardware_description(
        hardware, commit=plan["commit"]
    )
    if (
        not isinstance(plan["hardware"]["profile_sha256"], str)
        or SHA256.fullmatch(plan["hardware"]["profile_sha256"]) is None
        or plan["hardware"]["profile_sha256"] != hardware_profile_sha256
    ):
        raise RunnerError("hardware profile digest differs from the frozen plan")
    canary_manifest = read_bound_json_file(
        canary_path,
        {
            "sha256": plan["canary_manifest"]["sha256"],
            "bytes": plan["canary_manifest"]["bytes"],
        },
        "canary manifest",
    )
    if canary_manifest != build_canary_manifest(plan["commit"]):
        raise RunnerError("canary manifest differs from the canonical commit binding")
    leakage_audit.load_canaries(canary_path)
    if file_binding(canary_path) != {
        "sha256": plan["canary_manifest"]["sha256"],
        "bytes": plan["canary_manifest"]["bytes"],
    }:
        raise RunnerError("canary manifest changed while it was validated")
    configuration_manifest = read_bound_json_file(
        configuration_manifest_path,
        {
            "sha256": plan["configuration_manifest"]["sha256"],
            "bytes": plan["configuration_manifest"]["bytes"],
        },
        "configuration manifest",
    )
    exact_fields(
        configuration_manifest,
        {"version", "protocol", "commit", "configurations", "passed"},
        "configuration manifest",
    )
    if (
        configuration_manifest["version"] != VERSION
        or configuration_manifest["protocol"] != PROTOCOL
        or configuration_manifest["commit"] != plan["commit"]
        or configuration_manifest["passed"] is not True
    ):
        raise RunnerError("configuration manifest header is invalid")
    rows = configuration_manifest["configurations"]
    if not isinstance(rows, list) or len(rows) != len(PARTICIPANTS):
        raise RunnerError("configuration manifest does not cover the canonical matrix")
    participant_order = []
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            raise RunnerError(f"configuration[{index}] must be an object")
        participant_order.append(row.get("participants"))
    if participant_order != list(PARTICIPANTS):
        raise RunnerError("configuration manifest is reordered")
    configuration_digests: dict[int, str] = {}
    for index, row in enumerate(rows):
        record = exact_fields(
            row,
            {
                "participants",
                "validators_per_dataspace",
                "quorum",
                "mandatory_signed_rs16_da_rbc",
                "path",
                "sha256",
                "bytes",
            },
            f"configuration[{index}]",
        )
        if (
            record["participants"] not in PARTICIPANTS
            or record["validators_per_dataspace"] != VALIDATORS_PER_DATASPACE
            or record["quorum"] != QUORUM
            or record["mandatory_signed_rs16_da_rbc"] is not True
        ):
            raise RunnerError(f"configuration[{index}] weakens the release topology")
        config_path = regular_file_under(
            root,
            safe_relative_path(record["path"], "configuration.path"),
            f"configuration[{index}]",
        )
        if file_binding(config_path) != {
            "sha256": record["sha256"],
            "bytes": record["bytes"],
        }:
            raise RunnerError("configuration binding differs from the frozen plan")
        configuration_document = read_bound_json_file(
            config_path,
            {"sha256": record["sha256"], "bytes": record["bytes"]},
            f"configuration[{index}]",
        )
        expected_configuration = build_configuration(
            record["participants"],
            seeds=seeds,
            warmups=requirements["warmups"],
            measured=requirements["measured"],
        )
        if configuration_document != expected_configuration:
            raise RunnerError(
                f"configuration[{index}] differs from the canonical profile"
            )
        configuration_digests[record["participants"]] = record["sha256"]
    expected_jobs = build_jobs(
        configuration_digests,
        seeds,
        requirements["warmups"],
        requirements["measured"],
        canary_manifest,
    )
    if plan["jobs"] != expected_jobs:
        raise RunnerError("plan job matrix is incomplete, reordered, or altered")
    return plan, root


def expected_process_inventory_keys(
    participants: int,
) -> list[tuple[str, Any, Any]]:
    """Return canonical coordinator/global/participant process roles."""

    if participants not in PARTICIPANTS:
        raise RunnerError("process inventory uses an unsupported participant count")
    expected = [("coordinator", None, None)]
    expected.extend(
        ("global_validator", None, validator)
        for validator in range(GLOBAL_VALIDATORS)
    )
    expected.extend(
        ("dataspace_validator", dataspace, validator)
        for dataspace in range(participants)
        for validator in range(VALIDATORS_PER_DATASPACE)
    )
    return expected


def validate_process_inventory(
    value: Any, *, participants: int, commit: str, label: str
) -> None:
    """Require positive, healthy real-process identities for the exact topology."""

    if not isinstance(value, list):
        raise RunnerError(f"{label} must be a list")
    expected = expected_process_inventory_keys(participants)
    if len(value) != len(expected):
        raise RunnerError(
            f"{label} process topology mismatch; expected {len(expected)} rows"
        )
    pids: set[int] = set()
    validator_digests: set[str] = set()
    for index, item in enumerate(value):
        row = exact_fields(
            item,
            {
                "role",
                "dataspace_ordinal",
                "validator_ordinal",
                "pid",
                "executable_sha256",
                "revision",
                "health_observed",
            },
            f"{label}[{index}]",
        )
        identity = (
            row["role"],
            row["dataspace_ordinal"],
            row["validator_ordinal"],
        )
        if identity != expected[index]:
            raise RunnerError(
                f"{label} is incomplete, non-canonical, or reordered at row "
                f"{index}: expected {expected[index]!r}, got {identity!r}"
            )
        pid = positive_integer(row["pid"], f"{label}[{index}].pid")
        if pid in pids:
            raise RunnerError(f"{label} reuses PID {pid}")
        pids.add(pid)
        digest = row["executable_sha256"]
        if (
            not isinstance(digest, str)
            or SHA256.fullmatch(digest) is None
            or digest == "0" * 64
        ):
            raise RunnerError(f"{label}[{index}].executable_sha256 is invalid")
        if row["role"] != "coordinator":
            validator_digests.add(digest)
        if row["revision"] != commit:
            raise RunnerError(f"{label}[{index}] used a different source revision")
        require_true(row["health_observed"], f"{label}[{index}].health_observed")
    if len(validator_digests) != 1:
        raise RunnerError(f"{label} used different validator executable builds")


def validate_common_response(
    response: Any,
    *,
    plan: Mapping[str, Any],
    job: Mapping[str, Any],
) -> dict[str, Any]:
    """Validate the common real-process response envelope."""

    record = exact_fields(response, COMMON_RESPONSE_FIELDS, "harness response")
    invocation_nonce = job.get("invocation_nonce")
    if (
        not isinstance(invocation_nonce, str)
        or SHA256.fullmatch(invocation_nonce) is None
        or invocation_nonce == "0" * 64
    ):
        raise RunnerError("job lacks a valid per-invocation freshness nonce")
    if (
        record["version"] != VERSION
        or record["protocol"] != PROTOCOL
        or record["request_id"] != job["request_id"]
        or record["invocation_nonce"] != invocation_nonce
        or record["kind"] != job["kind"]
        or record["commit"] != plan["commit"]
        or record["hardware_sha256"] != plan["hardware"]["sha256"]
        or record["hardware_profile_sha256"]
        != plan["hardware"]["profile_sha256"]
        or record["configuration_sha256"] != job["configuration_sha256"]
        or record["participants"] != job["participants"]
    ):
        raise RunnerError("harness response does not bind the frozen request")
    require_true(record["passed"], "harness response.passed")
    require_true(
        record["mandatory_signed_rs16_da_rbc"],
        "harness response.mandatory_signed_rs16_da_rbc",
    )
    observations = bounded_integer(
        record["signed_rs16_da_observations"],
        1,
        MAX_OBSERVATION_COUNT,
        "harness response.signed_rs16_da_observations",
    )
    minimum_observations = minimum_signed_rs16_da_observations(
        job["participants"]
    )
    if observations < minimum_observations:
        raise RunnerError(
            "harness response.signed_rs16_da_observations must cover every "
            f"validator process (minimum {minimum_observations})"
        )
    require_true(
        record["authenticated_message_control"],
        "harness response.authenticated_message_control",
    )
    validate_process_inventory(
        record["process_inventory"],
        participants=job["participants"],
        commit=plan["commit"],
        label="harness response.process_inventory",
    )
    return record


def fault_control_record(
    record_id: str,
    participants: int,
    seed: int,
    run: int,
    collection: str,
    trial_index: int,
    trial: Mapping[str, Any],
) -> dict[str, Any]:
    """Build one verifier-compatible controller transcript row."""

    common = {
        "record": record_id,
        "participants": participants,
        "seed": seed,
        "run": run,
        "collection": collection,
        "trial_index": trial_index,
    }
    if collection == "loss_trials":
        return {
            **common,
            "phase": trial["phase"],
            "loss_percent": trial["loss_percent"],
            "control_acknowledged": trial["control_acknowledged"],
            "healed": trial["healed"],
            "converged": trial["converged"],
        }
    if collection == "phase_cut_partitions":
        return {
            **common,
            "cut": trial["cut"],
            "control_acknowledged": trial["control_acknowledged"],
            "delayed_delivery": trial["delayed_delivery"],
            "healed": trial["healed"],
            "converged": trial["converged"],
        }
    return {
        **common,
        "boundary": trial["boundary"],
        "process_restarted": trial["process_restarted"],
        "durable_state_reconciled": trial["durable_state_reconciled"],
        "converged": trial["converged"],
    }


def validate_prepare_qc_normalization(value: Any, label: str) -> None:
    """Require positive and negative evidence for quorum-equivalent QC encoding."""

    record = exact_fields(
        value,
        {
            "first_signer_subset",
            "second_signer_subset",
            "certified_body_sha256",
            "first_qc_sha256",
            "second_qc_sha256",
            "first_normalized_barrier_sha256",
            "second_normalized_barrier_sha256",
            "equivalent_subsets_accepted",
            "changed_body_rejected",
            "authority_index_binding_verified",
            "signed_body_binding_verified",
        },
        label,
    )
    if record["first_signer_subset"] != [0, 1, 2] or record[
        "second_signer_subset"
    ] != [0, 1, 3]:
        raise RunnerError(f"{label} must exercise two distinct valid 3-of-4 subsets")
    digest_fields = (
        "certified_body_sha256",
        "first_qc_sha256",
        "second_qc_sha256",
        "first_normalized_barrier_sha256",
        "second_normalized_barrier_sha256",
    )
    for field in digest_fields:
        digest = record[field]
        if (
            not isinstance(digest, str)
            or SHA256.fullmatch(digest) is None
            or digest == "0" * 64
        ):
            raise RunnerError(f"{label}.{field} must be a non-zero SHA-256")
    if record["first_qc_sha256"] == record["second_qc_sha256"]:
        raise RunnerError(f"{label} did not observe distinct QC encodings")
    if (
        record["first_normalized_barrier_sha256"]
        != record["second_normalized_barrier_sha256"]
    ):
        raise RunnerError(f"{label} did not normalize quorum-equivalent QCs")
    for field in (
        "equivalent_subsets_accepted",
        "changed_body_rejected",
        "authority_index_binding_verified",
        "signed_body_binding_verified",
    ):
        require_true(record[field], f"{label}.{field}")


def materialize_fault_response(
    response: Mapping[str, Any],
    *,
    plan: Mapping[str, Any],
    job: Mapping[str, Any],
    publication_root: Path,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Validate a fault response and bind it to transcript/capture JSONL files."""

    common = validate_common_response(response, plan=plan, job=job)
    payload = exact_fields(common["payload"], FAULT_PAYLOAD_FIELDS, "fault payload")
    participants = job["participants"]
    seed = job["seed"]
    run = job["run"]
    collections = (
        "loss_trials",
        "phase_cut_partitions",
        "crash_recoveries",
    )
    trial_fields = {
        "loss_trials": {
            "phase",
            "loss_percent",
            "control_acknowledged",
            "healed",
            "converged",
            "partial_visibility_observed",
        },
        "phase_cut_partitions": {
            "cut",
            "control_acknowledged",
            "delayed_delivery",
            "healed",
            "converged",
            "partial_visibility_observed",
        },
        "crash_recoveries": {
            "boundary",
            "process_restarted",
            "durable_state_reconciled",
            "converged",
            "partial_visibility_observed",
        },
    }
    control_rows: list[dict[str, Any]] = []
    capture_rows: list[dict[str, Any]] = []
    prepared: dict[str, list[dict[str, Any]]] = {}
    atomicity = payload["atomicity"]
    if not isinstance(atomicity, dict):
        raise RunnerError("fault payload.atomicity must be an object")
    validate_prepare_qc_normalization(
        payload["prepare_qc_normalization"],
        "fault payload.prepare_qc_normalization",
    )
    for collection in collections:
        trials = payload[collection]
        if not isinstance(trials, list):
            raise RunnerError(f"fault payload.{collection} must be a list")
        prepared[collection] = []
        for index, item in enumerate(trials):
            trial = exact_fields(
                item, trial_fields[collection], f"fault payload.{collection}[{index}]"
            )
            record_id = (
                f"n{participants}:s{seed}:r{run}:{collection}:{index}"
            )
            control_rows.append(
                fault_control_record(
                    record_id,
                    participants,
                    seed,
                    run,
                    collection,
                    index,
                    trial,
                )
            )
            capture_rows.append(
                {
                    "record": record_id,
                    "participants": participants,
                    "seed": seed,
                    "run": run,
                    "collection": collection,
                    "trial_index": index,
                    "continuous_checks": atomicity.get("continuous_checks"),
                    "partial_visibility_observed": trial[
                        "partial_visibility_observed"
                    ],
                    "partial_spendable_observations": atomicity.get(
                        "partial_spendable_observations"
                    ),
                    "converged": trial["converged"],
                }
            )
            prepared[collection].append(dict(trial))
    stem = f"n{participants}-s{seed}-r{run}"
    control_path = publication_root / "fault" / "control" / f"{stem}.jsonl"
    capture_path = publication_root / "fault" / "observations" / f"{stem}.jsonl"
    write_jsonl(control_path, control_rows)
    write_jsonl(capture_path, capture_rows)
    control_binding = file_binding(control_path)
    capture_binding = file_binding(capture_path)
    for collection in collections:
        for index, trial in enumerate(prepared[collection]):
            record_id = f"n{participants}:s{seed}:r{run}:{collection}:{index}"
            trial.update(
                {
                    "control_transcript_sha256": control_binding["sha256"],
                    "control_transcript_record": record_id,
                    "observation_capture_sha256": capture_binding["sha256"],
                    "observation_capture_record": record_id,
                }
            )
    raw = {
        "version": VERSION,
        "protocol": PROTOCOL,
        "commit": plan["commit"],
        "hardware_sha256": plan["hardware"]["sha256"],
        "configuration_sha256": job["configuration_sha256"],
        "participants": participants,
        "seed": seed,
        "run": run,
        "validators_per_dataspace": VALIDATORS_PER_DATASPACE,
        "quorum": QUORUM,
        "mandatory_signed_rs16_da_rbc": True,
        "authenticated_message_control": True,
        "committee_validator_restarts": payload[
            "committee_validator_restarts"
        ],
        "maximum_simultaneously_unavailable_per_committee": payload[
            "maximum_simultaneously_unavailable_per_committee"
        ],
        "quorum_progress_with_one_unavailable": payload[
            "quorum_progress_with_one_unavailable"
        ],
        "coordinator_restarted": payload["coordinator_restarted"],
        "global_node_restarted": payload["global_node_restarted"],
        "loss_trials": prepared["loss_trials"],
        "phase_cut_partitions": prepared["phase_cut_partitions"],
        "crash_recoveries": prepared["crash_recoveries"],
        "atomicity": payload["atomicity"],
        "all_nodes_converged": payload["all_nodes_converged"],
    }
    try:
        fault_report.parse_run(raw, f"harness:{job['request_id']}")
    except fault_report.FaultEvidenceError as error:
        raise RunnerError(f"fault harness result is incomplete: {error}") from error
    artifacts = [
        {
            "kind": "operator_log",
            **file_binding(control_path, relative_to=publication_root),
        },
        {
            "kind": "sanitized_capture",
            **file_binding(capture_path, relative_to=publication_root),
        },
    ]
    return raw, artifacts


def materialize_benchmark_response(
    response: Mapping[str, Any],
    *,
    plan: Mapping[str, Any],
    job: Mapping[str, Any],
) -> dict[str, Any]:
    """Validate and normalize one real-process benchmark measurement."""

    common = validate_common_response(response, plan=plan, job=job)
    expected = {
        "stages_ms",
        *benchmark_report.RESOURCE_FIELDS,
        *BENCHMARK_CORRECTNESS_FIELDS,
    }
    payload = exact_fields(common["payload"], expected, "benchmark payload")
    profile = job["profile"]
    stages = payload["stages_ms"]
    required_stages = (
        benchmark_report.REQUIRED_PRIVATE_STAGES
        if profile == "private"
        else ("global_finality", "end_to_end")
    )
    if not isinstance(stages, dict) or set(stages) != set(required_stages):
        raise RunnerError("benchmark stage inventory is incomplete")
    normalized_stages = {
        stage: finite_nonnegative(value, f"benchmark.{stage}")
        for stage, value in stages.items()
    }
    if normalized_stages["end_to_end"] <= 0:
        raise RunnerError("benchmark end-to-end latency must be positive")
    resources = {
        field: finite_nonnegative(payload[field], f"benchmark.{field}")
        for field in benchmark_report.RESOURCE_FIELDS
    }
    for field in (
        "throughput_bundles_per_second",
        "cpu_seconds",
        "peak_rss_bytes",
        "network_bytes",
        "receipt_bytes",
    ):
        if resources[field] <= 0:
            raise RunnerError(f"benchmark {field} must be positive")
    if profile == "private" and resources["proof_bytes"] <= 0:
        raise RunnerError("private benchmark proof_bytes must be positive")
    require_true(
        payload["finalized_receipt_observed"],
        "benchmark.finalized_receipt_observed",
    )
    if payload["successful_leg_applications"] != job["participants"]:
        raise RunnerError("benchmark successful leg count does not match participants")
    require_true(
        payload["each_leg_applied_exactly_once"],
        "benchmark.each_leg_applied_exactly_once",
    )
    if payload["partial_visible_observations"] != 0:
        raise RunnerError("benchmark observed a partially visible bundle")
    if payload["partial_spendable_observations"] != 0:
        raise RunnerError("benchmark observed a partially spendable bundle")
    raw = {
        "version": VERSION,
        "protocol": PROTOCOL,
        "commit": plan["commit"],
        "hardware_sha256": plan["hardware"]["sha256"],
        "hardware_profile_sha256": plan["hardware"]["profile_sha256"],
        "configuration_sha256": job["configuration_sha256"],
        "profile": profile,
        "participants": job["participants"],
        "seed": job["seed"],
        "run": job["run"],
        "warmup": job["warmup"],
        "stages_ms": normalized_stages,
        **resources,
    }
    try:
        benchmark_report.parse_sample(raw, f"harness:{job['request_id']}")
    except benchmark_report.EvidenceError as error:
        raise RunnerError(f"benchmark harness result is invalid: {error}") from error
    return raw


def validate_leakage_response(
    response: Mapping[str, Any],
    *,
    plan: Mapping[str, Any],
    job: Mapping[str, Any],
    evidence_dir: Path,
) -> tuple[
    dict[str, int], list[tuple[str, Path, dict[str, Any]]]
]:
    """Validate one secret-only differential capture and its file inventory."""

    common = validate_common_response(response, plan=plan, job=job)
    payload = exact_fields(
        common["payload"],
        {
            "variant",
            "canaries_injected",
            "canary_commitments",
            "only_secret_fields_changed",
            "capture_complete",
            "finalized_receipt_observed",
            "successful_leg_applications",
            "each_leg_applied_exactly_once",
            "partial_visible_observations",
            "partial_spendable_observations",
            "artifacts",
            "message_counts",
        },
        "leakage payload",
    )
    if payload["variant"] != job["variant"]:
        raise RunnerError("leakage response changed the requested variant")
    if payload["canaries_injected"] != job["canary_names"]:
        raise RunnerError("leakage harness did not inject every requested canary")
    if payload["canary_commitments"] != job["canary_commitments"]:
        raise RunnerError("leakage harness canary commitments do not match the request")
    for field in (
        "only_secret_fields_changed",
        "capture_complete",
        "finalized_receipt_observed",
        "each_leg_applied_exactly_once",
    ):
        require_true(payload[field], f"leakage.{field}")
    if payload["successful_leg_applications"] != PRIMARY_PARTICIPANTS:
        raise RunnerError("leakage run did not apply every primary leg")
    if payload["partial_visible_observations"] != 0:
        raise RunnerError("leakage run observed partial visibility")
    if payload["partial_spendable_observations"] != 0:
        raise RunnerError("leakage run observed partial spendability")
    counts = payload["message_counts"]
    if not isinstance(counts, dict) or set(counts) != set(
        leakage_audit.REQUIRED_COUNT_CHANNELS
    ):
        raise RunnerError("leakage message-count inventory is incomplete")
    normalized_counts = {
        channel: bounded_integer(
            counts[channel],
            1,
            MAX_OBSERVATION_COUNT,
            f"leakage.counts.{channel}",
        )
        for channel in leakage_audit.REQUIRED_COUNT_CHANNELS
    }
    artifacts = payload["artifacts"]
    expected_surfaces = sorted(SURFACE_FILES)
    if not isinstance(artifacts, list) or len(artifacts) != len(expected_surfaces):
        raise RunnerError("leakage artifacts must cover every required surface")
    if evidence_dir.is_symlink() or not evidence_dir.is_dir():
        raise RunnerError("leakage evidence root must be a regular directory")
    by_surface: dict[str, tuple[Path, dict[str, Any]]] = {}
    total_bytes = 0
    for index, item in enumerate(artifacts):
        row = exact_fields(
            item,
            {"surface", "relative_name", "sha256", "bytes"},
            f"leakage.artifacts[{index}]",
        )
        surface = row["surface"]
        if surface != expected_surfaces[index]:
            raise RunnerError(
                "leakage surfaces must be complete and canonically ordered"
            )
        relative = safe_relative_path(
            row["relative_name"], f"leakage.artifacts[{index}].relative_name"
        )
        if relative.as_posix() != SURFACE_FILES[surface]:
            raise RunnerError(
                f"leakage surface {surface} used a non-canonical filename"
            )
        source = regular_file_under(
            evidence_dir, relative, f"leakage surface {surface}"
        )
        binding = file_binding(source)
        if binding["bytes"] == 0:
            raise RunnerError(f"leakage surface {surface} must not be empty")
        if binding["bytes"] > leakage_audit.DEFAULT_MAX_FILE_BYTES:
            raise RunnerError(f"leakage surface {surface} exceeds the file-size bound")
        total_bytes += binding["bytes"]
        if total_bytes > leakage_audit.DEFAULT_MAX_TOTAL_BYTES:
            raise RunnerError("leakage capture exceeds the total-size bound")
        if binding != {"sha256": row["sha256"], "bytes": row["bytes"]}:
            raise RunnerError(f"leakage surface {surface} binding is false")
        by_surface[surface] = (source, binding)
    if set(by_surface) != set(SURFACE_FILES):
        raise RunnerError("leakage capture does not contain every required surface")
    declared_names = {SURFACE_FILES[surface] for surface in SURFACE_FILES}
    try:
        actual_entries = list(evidence_dir.iterdir())
    except OSError as error:
        raise RunnerError(f"cannot enumerate leakage evidence: {error}") from error
    if (
        {entry.name for entry in actual_entries} != declared_names
        or any(entry.is_symlink() or not entry.is_file() for entry in actual_entries)
    ):
        raise RunnerError("leakage evidence directory contains an undeclared file")
    return normalized_counts, [
        (surface, by_surface[surface][0], by_surface[surface][1])
        for surface in expected_surfaces
    ]


def build_request(
    plan: Mapping[str, Any],
    plan_root: Path,
    job: Mapping[str, Any],
) -> dict[str, Any]:
    """Build one exact harness request, including secret canaries only when needed."""

    configuration_manifest_path = regular_file_under(
        plan_root,
        safe_relative_path(
            plan["configuration_manifest"]["path"],
            "plan.configuration_manifest.path",
        ),
        "plan.configuration_manifest",
    )
    configuration_manifest = read_bound_json_file(
        configuration_manifest_path,
        {
            "sha256": plan["configuration_manifest"]["sha256"],
            "bytes": plan["configuration_manifest"]["bytes"],
        },
        "configuration manifest",
    )
    matching_configurations = [
        row
        for row in configuration_manifest["configurations"]
        if row["participants"] == job["participants"]
    ]
    if len(matching_configurations) != 1:
        raise RunnerError("request cannot resolve one participant configuration")
    configuration_reference = matching_configurations[0]
    configuration_path = regular_file_under(
        plan_root,
        safe_relative_path(
            configuration_reference["path"], "configuration.path"
        ),
        "request configuration",
    )
    configuration = read_bound_json_file(
        configuration_path,
        {
            "sha256": job["configuration_sha256"],
            "bytes": configuration_reference["bytes"],
        },
        "request configuration",
    )
    invocation_nonce = job.get("invocation_nonce")
    if (
        not isinstance(invocation_nonce, str)
        or SHA256.fullmatch(invocation_nonce) is None
        or invocation_nonce == "0" * 64
    ):
        raise RunnerError("request requires a random 256-bit invocation nonce")
    request: dict[str, Any] = {
        "version": VERSION,
        "protocol": PROTOCOL,
        "request_id": job["request_id"],
        "invocation_nonce": invocation_nonce,
        "kind": job["kind"],
        "commit": plan["commit"],
        "hardware_sha256": plan["hardware"]["sha256"],
        "hardware_profile_sha256": plan["hardware"]["profile_sha256"],
        "configuration_sha256": job["configuration_sha256"],
        "participants": job["participants"],
        "validators_per_dataspace": VALIDATORS_PER_DATASPACE,
        "global_validators": GLOBAL_VALIDATORS,
        "quorum": QUORUM,
        "mandatory_signed_rs16_da_rbc": True,
        "minimum_signed_rs16_da_observations": (
            minimum_signed_rs16_da_observations(job["participants"])
        ),
        "authenticated_message_control": True,
        "seed": job["seed"],
        "run": job["run"],
        "configuration": configuration,
    }
    if job["kind"] == "fault":
        request["payload"] = {
            "loss_phases": list(fault_report.REQUIRED_LOSS_PHASES),
            "loss_percentages": list(fault_report.REQUIRED_LOSS_PERCENTAGES),
            "phase_cuts": list(fault_report.REQUIRED_PHASE_CUTS),
            "crash_boundaries": list(fault_report.REQUIRED_CRASH_BOUNDARIES),
            "committee_validator_restarts": list(range(job["participants"])),
            "restart_coordinator": True,
            "restart_global_node": True,
            "maximum_simultaneously_unavailable_per_committee": 1,
            "continuous_atomicity_checks": True,
            "prepare_qc_normalization": {
                "first_signer_subset": [0, 1, 2],
                "second_signer_subset": [0, 1, 3],
                "accept_equivalent_subsets_only_for_identical_body": True,
                "bind_authority_indices": True,
                "bind_every_signed_body": True,
                "reject_changed_certified_body": True,
            },
        }
    elif job["kind"] == "benchmark":
        request["payload"] = {
            "profile": job["profile"],
            "warmup": job["warmup"],
            "stages": list(
                benchmark_report.REQUIRED_PRIVATE_STAGES
                if job["profile"] == "private"
                else ("global_finality", "end_to_end")
            ),
            "resources": list(benchmark_report.RESOURCE_FIELDS),
        }
    else:
        canary_path = regular_file_under(
            plan_root,
            safe_relative_path(
                plan["canary_manifest"]["path"], "plan.canary_manifest.path"
            ),
            "plan.canary_manifest",
        )
        manifest = read_bound_json_file(
            canary_path,
            {
                "sha256": plan["canary_manifest"]["sha256"],
                "bytes": plan["canary_manifest"]["bytes"],
            },
            "canary manifest",
        )
        selected_canaries = canaries_for_variant(manifest, job["variant"])
        selected_commitments = {
            entry["name"]: object_digest(entry) for entry in selected_canaries
        }
        if (
            [entry["name"] for entry in selected_canaries]
            != job["canary_names"]
            or selected_commitments != job["canary_commitments"]
        ):
            raise RunnerError("request canaries differ from the frozen job binding")
        request["payload"] = {
            "variant": job["variant"],
            "canaries": selected_canaries,
            "canary_commitments": job["canary_commitments"],
            "only_secret_fields_change": True,
            "capture_surfaces": [
                {"surface": surface, "relative_name": SURFACE_FILES[surface]}
                for surface in sorted(SURFACE_FILES)
            ],
            "message_count_channels": list(
                leakage_audit.REQUIRED_COUNT_CHANNELS
            ),
        }
    return request


def invoke_harness(
    harness: Path,
    request: Mapping[str, Any],
    *,
    timeout_seconds: int,
    expected_harness_binding: Mapping[str, Any] | None = None,
) -> tuple[
    dict[str, Any],
    Path,
    Path,
    dict[str, Any],
    tempfile.TemporaryDirectory[str],
]:
    """Invoke the frozen harness through the strict file protocol.

    The executable receives ``--aps-request``, ``--aps-response``, and
    ``--aps-evidence-dir``.  It must exit zero, create exactly one response
    JSON file, and (for leakage jobs) write the declared capture files beneath
    the evidence directory.  Stdout/stderr never substitute for the response.
    The returned temporary directory remains owned by the caller until its
    ``cleanup`` method is called.
    """

    if timeout_seconds <= 0:
        raise RunnerError("harness timeout must be positive")
    if (
        expected_harness_binding is not None
        and verify_harness(harness) != dict(expected_harness_binding)
    ):
        raise RunnerError("harness executable changed before invocation")
    temporary = tempfile.TemporaryDirectory(prefix="aps-harness-")
    root = Path(temporary.name)
    request_path = root / "request.json"
    response_path = root / "response.json"
    evidence_dir = root / "evidence"
    stdout_path = root / "stdout.log"
    stderr_path = root / "stderr.log"
    evidence_dir.mkdir()
    write_json(request_path, request)
    command = [
        str(harness),
        "--aps-request",
        str(request_path),
        "--aps-response",
        str(response_path),
        "--aps-evidence-dir",
        str(evidence_dir),
    ]
    try:
        with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
            completed = subprocess.run(
                command,
                check=False,
                stdout=stdout,
                stderr=stderr,
                timeout=timeout_seconds,
            )
    except (OSError, subprocess.TimeoutExpired) as error:
        temporary.cleanup()
        raise RunnerError(f"real-process harness invocation failed: {error}") from error
    if (
        expected_harness_binding is not None
        and verify_harness(harness) != dict(expected_harness_binding)
    ):
        temporary.cleanup()
        raise RunnerError("harness executable changed during invocation")
    if completed.returncode != 0:
        try:
            with stderr_path.open("rb") as stream:
                stream.seek(max(0, stderr_path.stat().st_size - 2_000))
                stderr_tail = stream.read().decode("utf-8", errors="replace")
        except OSError:
            stderr_tail = "<stderr unavailable>"
        temporary.cleanup()
        raise RunnerError(
            f"real-process harness exited {completed.returncode}: {stderr_tail}"
        )
    if response_path.is_symlink() or not response_path.is_file():
        temporary.cleanup()
        raise RunnerError("harness exited successfully without a regular response file")
    response_binding = file_binding(response_path)
    if response_binding["bytes"] > MAX_HARNESS_RESPONSE_BYTES:
        temporary.cleanup()
        raise RunnerError("harness response exceeds the bounded response size")
    try:
        response = read_bound_json_file(
            response_path, response_binding, "harness response"
        )
    except RunnerError as error:
        temporary.cleanup()
        raise RunnerError(str(error)) from error
    if request.get("kind") != "leakage" and any(evidence_dir.iterdir()):
        temporary.cleanup()
        raise RunnerError("non-leakage harness wrote undeclared evidence files")
    expected_root_names = {
        "request.json",
        "response.json",
        "evidence",
        "stdout.log",
        "stderr.log",
    }
    if {entry.name for entry in root.iterdir()} != expected_root_names:
        temporary.cleanup()
        raise RunnerError("harness wrote an undeclared protocol file")
    return response, evidence_dir, response_path, response_binding, temporary


def copy_bound_file(
    source: Path,
    destination: Path,
    *,
    expected: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Copy one bound regular file and verify source and destination identity."""

    before = file_binding(source)
    if expected is not None and before != dict(expected):
        raise RunnerError(f"source evidence changed before copy: {source}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor, metadata = _open_regular_nofollow(source)
    digest = hashlib.sha256()
    copied_bytes = 0
    try:
        with os.fdopen(descriptor, "rb") as input_stream, destination.open(
            "xb"
        ) as output_stream:
            while chunk := input_stream.read(1024 * 1024):
                digest.update(chunk)
                copied_bytes += len(chunk)
                output_stream.write(chunk)
    except OSError as error:
        raise RunnerError(f"cannot copy bound evidence {source}: {error}") from error
    copied_binding = {"sha256": digest.hexdigest(), "bytes": copied_bytes}
    if metadata.st_size != copied_bytes or copied_binding != before:
        raise RunnerError(f"source evidence changed while copied: {source}")
    if file_binding(destination) != before:
        raise RunnerError(f"copied evidence changed bytes: {source}")
    return before


def archive_harness_response(
    response_path: Path,
    response_binding: Mapping[str, Any],
    publication_root: Path,
    request_id: str,
) -> dict[str, Any]:
    """Archive the exact validated harness-response bytes as an operator log."""

    path = publication_root / "harness-responses" / f"{request_id}.json"
    copy_bound_file(response_path, path, expected=response_binding)
    return {
        "kind": "operator_log",
        **file_binding(path, relative_to=publication_root),
    }


def write_benchmark_csv(path: Path, rows: Sequence[Mapping[str, Any]]) -> None:
    """Write a flat raw CSV companion without replacing the canonical JSONL."""

    stage_names = list(benchmark_report.REQUIRED_PRIVATE_STAGES)
    fieldnames = [
        "version",
        "protocol",
        "commit",
        "hardware_sha256",
        "hardware_profile_sha256",
        "configuration_sha256",
        "profile",
        "participants",
        "seed",
        "run",
        "warmup",
        *[f"stage_ms_{stage}" for stage in stage_names],
        *benchmark_report.RESOURCE_FIELDS,
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fieldnames)
        writer.writeheader()
        for row in rows:
            flattened = {key: row[key] for key in fieldnames if key in row}
            for stage in stage_names:
                flattened[f"stage_ms_{stage}"] = row["stages_ms"].get(stage, "")
            writer.writerow(flattened)


def write_fault_csv(path: Path, rows: Sequence[Mapping[str, Any]]) -> None:
    """Write one flat row for every controlled fault trial."""

    fieldnames = [
        "participants",
        "seed",
        "run",
        "collection",
        "trial_index",
        "fault",
        "control_acknowledged",
        "healed",
        "converged",
        "partial_visibility_observed",
        "control_transcript_sha256",
        "control_transcript_record",
        "observation_capture_sha256",
        "observation_capture_record",
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fieldnames)
        writer.writeheader()
        for row in rows:
            for collection in (
                "loss_trials",
                "phase_cut_partitions",
                "crash_recoveries",
            ):
                for index, trial in enumerate(row[collection]):
                    if collection == "loss_trials":
                        fault = f"{trial['phase']}:{trial['loss_percent']}"
                        acknowledged = trial["control_acknowledged"]
                        healed = trial["healed"]
                    elif collection == "phase_cut_partitions":
                        fault = trial["cut"]
                        acknowledged = trial["control_acknowledged"]
                        healed = trial["healed"]
                    else:
                        fault = trial["boundary"]
                        acknowledged = trial["process_restarted"]
                        healed = trial["durable_state_reconciled"]
                    writer.writerow(
                        {
                            "participants": row["participants"],
                            "seed": row["seed"],
                            "run": row["run"],
                            "collection": collection,
                            "trial_index": index,
                            "fault": fault,
                            "control_acknowledged": acknowledged,
                            "healed": healed,
                            "converged": trial["converged"],
                            "partial_visibility_observed": trial[
                                "partial_visibility_observed"
                            ],
                            "control_transcript_sha256": trial[
                                "control_transcript_sha256"
                            ],
                            "control_transcript_record": trial[
                                "control_transcript_record"
                            ],
                            "observation_capture_sha256": trial[
                                "observation_capture_sha256"
                            ],
                            "observation_capture_record": trial[
                                "observation_capture_record"
                            ],
                        }
                    )


def differential_pair_manifest(
    publication_root: Path, commit: str
) -> dict[str, Any]:
    """Bind every left/right privacy surface in canonical order."""

    pairs = []
    for surface in sorted(SURFACE_FILES):
        relative_name = SURFACE_FILES[surface]
        left = publication_root / "leakage" / "left" / relative_name
        right = publication_root / "leakage" / "right" / relative_name
        left_binding = file_binding(left, relative_to=publication_root)
        right_binding = file_binding(right, relative_to=publication_root)
        if (
            surface in REQUIRED_DIFFERENTIAL_STATE_CHANGES
            and left_binding["sha256"] == right_binding["sha256"]
        ):
            raise RunnerError(
                f"differential surface {surface} did not change when secrets changed"
            )
        pairs.append(
            {
                "surface": surface,
                "relative_name": relative_name,
                "left": left_binding,
                "right": right_binding,
            }
        )
    return {
        "version": VERSION,
        "protocol": PROTOCOL,
        "commit": commit,
        "left_root": "leakage/left",
        "right_root": "leakage/right",
        "pairs": pairs,
        "passed": True,
    }


def validate_publication_fragment(
    publication_root: Path,
    artifacts: Sequence[Mapping[str, Any]],
    *,
    commit: str,
) -> None:
    """Replay every strict final-bundle validator applicable to this fragment."""

    unknown_kinds = {
        artifact["kind"]
        for artifact in artifacts
        if artifact["kind"] not in release_evidence.REQUIRED_ARTIFACT_KINDS
    }
    if unknown_kinds:
        raise RunnerError(
            "publication fragment uses unknown artifact kinds: "
            f"{sorted(unknown_kinds)}"
        )
    artifact_objects = [
        release_evidence.Artifact(
            kind=artifact["kind"],
            path=PurePosixPath(artifact["path"]),
            sha256=artifact["sha256"],
            bytes=artifact["bytes"],
        )
        for artifact in artifacts
    ]
    by_path = {artifact.path: artifact for artifact in artifact_objects}
    if len(by_path) != len(artifact_objects):
        raise RunnerError("publication fragment contains duplicate paths")
    hardware = [
        artifact
        for artifact in artifact_objects
        if artifact.kind == "hardware_description"
    ]
    configurations = [
        artifact
        for artifact in artifact_objects
        if artifact.kind == "configuration_manifest"
    ]
    fault_raw = [
        artifact
        for artifact in artifact_objects
        if artifact.kind == "real_network_fault_raw"
    ]
    fault_reports = [
        artifact
        for artifact in artifact_objects
        if artifact.kind == "real_network_fault_report"
    ]
    benchmark_raw = [
        artifact for artifact in artifact_objects if artifact.kind == "benchmark_raw"
    ]
    benchmark_reports = [
        artifact for artifact in artifact_objects if artifact.kind == "benchmark_report"
    ]
    leakage_reports = [
        artifact for artifact in artifact_objects if artifact.kind == "leakage_report"
    ]
    if not (
        len(hardware)
        == len(configurations)
        == len(fault_reports)
        == len(benchmark_reports)
        == len(leakage_reports)
        == 1
        and fault_raw
        and benchmark_raw
    ):
        raise RunnerError(
            "publication fragment lacks a unique required report or raw input"
        )
    hardware_path = publication_root.joinpath(*hardware[0].path.parts)
    hardware_profile_sha256 = release_evidence._validate_hardware_description(
        hardware_path, commit=commit
    )
    configuration_digests = release_evidence._validate_configuration_manifest(
        publication_root.joinpath(*configurations[0].path.parts),
        commit=commit,
        artifacts_by_path=by_path,
    )
    release_evidence._validate_fault_report(
        publication_root.joinpath(*fault_reports[0].path.parts),
        raw_artifacts=fault_raw,
        artifacts=artifact_objects,
        root=publication_root,
        commit=commit,
        hardware_sha256=hardware[0].sha256,
        configuration_sha256_by_participants=configuration_digests,
    )
    release_evidence._validate_leakage_report(
        publication_root.joinpath(*leakage_reports[0].path.parts),
        artifact_objects,
        publication_root,
        commit,
    )
    benchmark_paths = [
        publication_root.joinpath(*artifact.path.parts) for artifact in benchmark_raw
    ]
    raw_buckets = release_evidence._load_benchmark_raw(
        benchmark_paths,
        commit,
        hardware[0].sha256,
        hardware_profile_sha256,
        configuration_digests,
    )
    release_evidence._validate_benchmark_report(
        publication_root.joinpath(*benchmark_reports[0].path.parts),
        raw_buckets,
        benchmark_paths,
        commit,
        hardware[0].sha256,
        hardware_profile_sha256,
        configuration_digests,
    )


def execute_plan(
    plan_path: Path,
    output_dir: Path,
    *,
    source_root: Path,
    harness: Path,
    timeout_seconds: int,
) -> Path:
    """Execute the complete frozen campaign and publish only a clean result."""

    if output_dir.exists():
        raise RunnerError(f"execution output already exists: {output_dir}")
    require_external_output(output_dir, source_root)
    plan, plan_root = load_plan(plan_path)
    plan_file_binding = file_binding(plan_path)
    verify_source_checkout(source_root, plan["commit"])
    harness_binding = verify_harness(harness)
    if harness_binding != plan["harness"]:
        raise RunnerError("harness executable differs from the frozen plan")

    parent = output_dir.parent.resolve()
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="aps-execute-", dir=parent) as temporary:
        staging = Path(temporary)
        publication = staging / "publication"
        publication.mkdir()
        artifacts: list[dict[str, Any]] = []
        publication_plan = publication / "run-plan-v1.json"
        copy_bound_file(plan_path, publication_plan, expected=plan_file_binding)
        artifacts.append(
            {
                "kind": "operator_log",
                **file_binding(publication_plan, relative_to=publication),
            }
        )
        foundation = (
            ("hardware", "hardware-description-v1.json", "hardware_description"),
            ("canary_manifest", "canary-manifest-v1.json", "canary_manifest"),
            (
                "configuration_manifest",
                "configuration-manifest-v1.json",
                "configuration_manifest",
            ),
        )
        for plan_key, destination_name, kind in foundation:
            source = regular_file_under(
                plan_root,
                safe_relative_path(plan[plan_key]["path"], plan_key),
                f"plan.{plan_key}",
            )
            destination = publication / destination_name
            copy_bound_file(
                source,
                destination,
                expected={
                    "sha256": plan[plan_key]["sha256"],
                    "bytes": plan[plan_key]["bytes"],
                },
            )
            artifacts.append(
                {"kind": kind, **file_binding(destination, relative_to=publication)}
            )
        configuration_manifest = read_json_file(
            publication / "configuration-manifest-v1.json",
            "publication configuration manifest",
        )
        for row in configuration_manifest["configurations"]:
            relative = safe_relative_path(row["path"], "configuration.path")
            source = regular_file_under(
                plan_root, relative, "plan configuration"
            )
            destination = publication.joinpath(*relative.parts)
            copy_bound_file(
                source,
                destination,
                expected={"sha256": row["sha256"], "bytes": row["bytes"]},
            )
            artifacts.append(
                {
                    "kind": "configuration",
                    **file_binding(destination, relative_to=publication),
                }
            )

        fault_rows: list[dict[str, Any]] = []
        benchmark_rows: list[dict[str, Any]] = []
        leakage_counts: dict[str, dict[str, int]] = {}
        for ordinal, job in enumerate(plan["jobs"], 1):
            execution_job = {
                **job,
                "invocation_nonce": os.urandom(32).hex(),
            }
            request = build_request(plan, plan_root, execution_job)
            (
                response,
                evidence_dir,
                response_path,
                response_binding,
                temporary_job,
            ) = invoke_harness(
                harness,
                request,
                timeout_seconds=timeout_seconds,
                expected_harness_binding=plan["harness"],
            )
            try:
                if job["kind"] == "fault":
                    raw, fault_artifacts = materialize_fault_response(
                        response,
                        plan=plan,
                        job=execution_job,
                        publication_root=publication,
                    )
                    fault_rows.append(raw)
                    artifacts.extend(fault_artifacts)
                elif job["kind"] == "benchmark":
                    benchmark_rows.append(
                        materialize_benchmark_response(
                            response, plan=plan, job=execution_job
                        )
                    )
                else:
                    counts, surfaces = validate_leakage_response(
                        response,
                        plan=plan,
                        job=execution_job,
                        evidence_dir=evidence_dir,
                    )
                    leakage_counts[job["variant"]] = counts
                    for surface, source, expected_binding in surfaces:
                        destination = (
                            publication
                            / "leakage"
                            / job["variant"]
                            / SURFACE_FILES[surface]
                        )
                        copy_bound_file(
                            source, destination, expected=expected_binding
                        )
                        artifacts.append(
                            {
                                "kind": surface,
                                **file_binding(destination, relative_to=publication),
                            }
                        )
                artifacts.append(
                    archive_harness_response(
                        response_path,
                        response_binding,
                        publication,
                        job["request_id"],
                    )
                )
            finally:
                temporary_job.cleanup()
            if ordinal % 25 == 0:
                print(
                    f"validated {ordinal}/{len(plan['jobs'])} real-process jobs",
                    file=sys.stderr,
                )

        if len(fault_rows) != len(PARTICIPANTS) * len(
            plan["requirements"]["seeds"]
        ):
            raise RunnerError("fault harness did not return the complete matrix")
        expected_benchmarks = len(PROFILES) * len(PARTICIPANTS) * (
            plan["requirements"]["warmups"] + plan["requirements"]["measured"]
        )
        if len(benchmark_rows) != expected_benchmarks:
            raise RunnerError("benchmark harness did not return the complete matrix")
        if set(leakage_counts) != {"left", "right"}:
            raise RunnerError(
                "leakage harness did not return both differential variants"
            )

        fault_raw_path = publication / "raw" / "faults.jsonl"
        write_jsonl(fault_raw_path, fault_rows)
        artifacts.append(
            {
                "kind": "real_network_fault_raw",
                **file_binding(fault_raw_path, relative_to=publication),
            }
        )
        fault_report_value = fault_report.build_report(
            fault_report.load_runs([fault_raw_path]),
            fault_report.input_bindings([fault_raw_path]),
        )
        fault_report_path = publication / "reports" / "fault-report-v1.json"
        write_json(fault_report_path, fault_report_value)
        artifacts.append(
            {
                "kind": "real_network_fault_report",
                **file_binding(fault_report_path, relative_to=publication),
            }
        )
        fault_csv = publication / "raw" / "faults.csv"
        write_fault_csv(fault_csv, fault_rows)
        artifacts.append(
            {"kind": "operator_log", **file_binding(fault_csv, relative_to=publication)}
        )

        benchmark_raw_path = publication / "raw" / "benchmarks.jsonl"
        write_jsonl(benchmark_raw_path, benchmark_rows)
        artifacts.append(
            {
                "kind": "benchmark_raw",
                **file_binding(benchmark_raw_path, relative_to=publication),
            }
        )
        samples = benchmark_report.load_jsonl([benchmark_raw_path])
        benchmark_report_value = benchmark_report.build_report(
            samples, plan["requirements"]["bootstrap_iterations"]
        )
        regressions: list[dict[str, Any]] = []
        if plan["benchmark_baseline"] is not None:
            baseline_path = regular_file_under(
                plan_root,
                safe_relative_path(
                    plan["benchmark_baseline"]["path"],
                    "plan.benchmark_baseline.path",
                ),
                "plan.benchmark_baseline",
            )
            baseline = validate_benchmark_baseline(
                read_bound_json_file(
                    baseline_path,
                    {
                        "sha256": plan["benchmark_baseline"]["sha256"],
                        "bytes": plan["benchmark_baseline"]["bytes"],
                    },
                    "benchmark baseline",
                ),
                "benchmark baseline",
            )
            try:
                regressions = benchmark_report.compare_baseline(
                    benchmark_report_value, baseline
                )
            except benchmark_report.EvidenceError as error:
                raise RunnerError(
                    f"benchmark baseline is incompatible: {error}"
                ) from error
        benchmark_report_value["regressions"] = regressions
        benchmark_report_value["passed"] = not regressions
        if regressions:
            raise RunnerError(
                "benchmark campaign exceeds the signed regression baseline"
            )
        benchmark_report_path = publication / "reports" / "benchmark-report-v1.json"
        write_json(benchmark_report_path, benchmark_report_value)
        artifacts.append(
            {
                "kind": "benchmark_report",
                **file_binding(benchmark_report_path, relative_to=publication),
            }
        )
        benchmark_csv = publication / "raw" / "benchmarks.csv"
        write_benchmark_csv(benchmark_csv, benchmark_rows)
        artifacts.append(
            {
                "kind": "operator_log",
                **file_binding(benchmark_csv, relative_to=publication),
            }
        )

        count_paths: dict[str, Path] = {}
        for variant in ("left", "right"):
            path = publication / "leakage" / f"message-counts-{variant}.json"
            write_json(
                path,
                {"version": VERSION, "channels": leakage_counts[variant]},
            )
            count_paths[variant] = path
            artifacts.append(
                {
                    "kind": "message_count_manifest",
                    **file_binding(path, relative_to=publication),
                }
            )
        pair_manifest_path = publication / "leakage" / "differential-pairs-v1.json"
        write_json(
            pair_manifest_path,
            differential_pair_manifest(publication, plan["commit"]),
        )
        artifacts.append(
            {
                "kind": "differential_pair_manifest",
                **file_binding(pair_manifest_path, relative_to=publication),
            }
        )

        scannable = [
            publication.joinpath(
                *safe_relative_path(artifact["path"], "artifact.path").parts
            )
            for artifact in artifacts
            if artifact["kind"] in release_evidence.REQUIRED_LEAKAGE_ARTIFACT_KINDS
            or artifact["kind"] == "message_count_manifest"
        ]
        leakage_report_value = leakage_audit.run_audit(
            publication / "canary-manifest-v1.json",
            scannable,
            differential_left=publication / "leakage" / "left",
            differential_right=publication / "leakage" / "right",
            message_counts_left=count_paths["left"],
            message_counts_right=count_paths["right"],
        )
        if leakage_report_value["passed"] is not True:
            raise RunnerError(
                "leakage audit found a canary, public-shape, size, or "
                "message-count mismatch"
            )
        leakage_report_path = publication / "reports" / "leakage-report-v1.json"
        write_json(leakage_report_path, leakage_report_value)
        artifacts.append(
            {
                "kind": "leakage_report",
                **file_binding(leakage_report_path, relative_to=publication),
            }
        )

        artifacts.sort(key=lambda item: item["path"])
        if len({artifact["path"] for artifact in artifacts}) != len(artifacts):
            raise RunnerError("publication fragment contains a duplicate artifact path")
        validate_publication_fragment(
            publication,
            artifacts,
            commit=plan["commit"],
        )
        fragment = {
            "version": VERSION,
            "protocol": PROTOCOL,
            "commit": plan["commit"],
            "publication_root": "publication",
            "real_process_campaign_complete": True,
            "publication_evidence": False,
            "benchmark_baseline": (
                None
                if plan["benchmark_baseline"] is None
                else {
                    "sha256": plan["benchmark_baseline"]["sha256"],
                    "bytes": plan["benchmark_baseline"]["bytes"],
                }
            ),
            "reason": (
                "This fragment covers real-network fault, benchmark, and leakage "
                "experiments only; the final DOI manifest and all independent release "
                "gates must be assembled and verified separately."
            ),
            "external_gates_not_covered": [
                "independent_audit",
                "doi_publication",
                "reproducible_build_and_sbom",
                "workspace_clippy_format_sdk_inventory_randomized_soak_privacy_reports",
            ],
            "artifacts": artifacts,
        }
        write_json(staging / "release-artifact-fragment-v1.json", fragment)
        if verify_harness(harness) != plan["harness"]:
            raise RunnerError("harness executable changed during the campaign")
        if file_binding(plan_path) != plan_file_binding:
            raise RunnerError("frozen run plan changed during the campaign")
        reloaded_plan, reloaded_root = load_plan(plan_path)
        if reloaded_plan != plan or reloaded_root != plan_root:
            raise RunnerError("frozen plan inputs changed during the campaign")
        verify_source_checkout(source_root, plan["commit"])
        staging.rename(output_dir)
    return output_dir / "release-artifact-fragment-v1.json"


def parse_seeds(value: str) -> tuple[int, ...]:
    """Parse comma-separated canonical campaign seeds."""

    try:
        seeds = tuple(int(item, 10) for item in value.split(",") if item != "")
    except ValueError as error:
        raise argparse.ArgumentTypeError(
            "seeds must be comma-separated integers"
        ) from error
    try:
        return verify_seed_policy(seeds)
    except RunnerError as error:
        raise argparse.ArgumentTypeError(str(error)) from error


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    """Parse the two-phase runner CLI."""

    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    plan = subparsers.add_parser(
        "plan", help="freeze non-evidence campaign scaffolding"
    )
    plan.add_argument("--output-dir", required=True, type=Path)
    plan.add_argument("--source-root", required=True, type=Path)
    plan.add_argument("--commit", required=True)
    plan.add_argument("--harness", required=True, type=Path)
    plan.add_argument("--hardware-description", required=True, type=Path)
    plan.add_argument(
        "--seeds",
        type=parse_seeds,
        default=tuple(range(MIN_FAULT_SEEDS)),
        help="sorted unique comma-separated integers (minimum ten)",
    )
    plan.add_argument("--warmups", type=int, default=MIN_WARMUPS)
    plan.add_argument("--measured", type=int, default=MIN_MEASURED)
    plan.add_argument(
        "--bootstrap-iterations", type=int, default=DEFAULT_BOOTSTRAP_ITERATIONS
    )
    plan.add_argument(
        "--benchmark-baseline",
        type=Path,
        help="optional passing V1 report for post-initial-release regression gates",
    )

    execute = subparsers.add_parser("execute", help="run every frozen real-process job")
    execute.add_argument("--plan", required=True, type=Path)
    execute.add_argument("--output-dir", required=True, type=Path)
    execute.add_argument("--source-root", required=True, type=Path)
    execute.add_argument("--harness", required=True, type=Path)
    execute.add_argument("--timeout-seconds", type=int, default=1_800)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    """Run the release experiment planner or executor."""

    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        if args.command == "plan":
            result = create_plan(
                args.output_dir,
                source_root=args.source_root,
                commit=args.commit,
                harness=args.harness,
                hardware_description=args.hardware_description,
                seeds=args.seeds,
                warmups=args.warmups,
                measured=args.measured,
                bootstrap_iterations=args.bootstrap_iterations,
                benchmark_baseline=args.benchmark_baseline,
            )
        else:
            result = execute_plan(
                args.plan,
                args.output_dir,
                source_root=args.source_root,
                harness=args.harness,
                timeout_seconds=args.timeout_seconds,
            )
    except (
        RunnerError,
        OSError,
        json.JSONDecodeError,
        fault_report.FaultEvidenceError,
        benchmark_report.EvidenceError,
        leakage_audit.AuditInputError,
        release_evidence.EvidenceError,
    ) as error:
        print(f"private-settlement release runner error: {error}", file=sys.stderr)
        return 2
    print(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
