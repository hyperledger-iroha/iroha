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
import signal
import stat
import struct
import subprocess
import sys
import tempfile
import time
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import private_settlement_benchmark_report as benchmark_report
import private_settlement_capture_split as capture_split
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
RAYON_WORKER_THREADS = 8
VALIDATOR_WORKER_THREADS = 4
CARGO_BUILD_JOBS = 1
CARGO_RELEASE_CODEGEN_UNITS = 1
CARGO_INCREMENTAL = False
PROFILES = ("private", "transparent_control")
PUBLIC_PARTICIPANT_VISIBILITY = "public"
RESTRICTED_PARTICIPANT_VISIBILITY = "restricted"
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
# Keep this timing contract synchronized with the production-like real-process
# fixture.  A fault job must first pay the governed private-profile activation
# delay and then advance four deliberately non-finalized crash trials past
# their bundle expiries.  This is a protocol floor, before startup, proof,
# restart, transaction, and polling overhead.
REAL_PROCESS_BLOCK_CADENCE_SECONDS = 4
PRIVACY_PROFILE_ACTIVATION_DELAY_BLOCKS = 300
FAULT_NONFINALIZED_EXPIRY_TRIALS = 4
FAULT_BUNDLE_EXPIRY_BLOCKS = 96
FAULT_EXPIRY_ADVANCE_BLOCKS = FAULT_BUNDLE_EXPIRY_BLOCKS + 1
FAULT_HARNESS_PROTOCOL_FLOOR_SECONDS = (
    PRIVACY_PROFILE_ACTIVATION_DELAY_BLOCKS
    + FAULT_NONFINALIZED_EXPIRY_TRIALS * FAULT_EXPIRY_ADVANCE_BLOCKS
) * REAL_PROCESS_BLOCK_CADENCE_SECONDS
# Leave substantial headroom above the deterministic floor for 16 processes,
# native proofs, restart recovery, and control acknowledgements.
DEFAULT_HARNESS_TIMEOUT_SECONDS = 7_200
GIT_OBJECT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
SHA256 = re.compile(r"[0-9a-f]{64}")
IROHA_HASH_LITERAL = re.compile(r"hash:([0-9A-F]{64})#[0-9A-F]{4}")

SURFACE_FILES: Mapping[str, str] = {
    "block_wire_capture": "block-wire.bin",
    "event_capture": "events.json",
    "kura_artifact": "kura.bin",
    "merge_artifact": "merge.bin",
    "operator_log": "operator.json",
    "public_p2p_capture": "public-p2p.pcapng",
    "query_capture": "queries.json",
    "restricted_audit_source": "restricted-audit-sources.bin",
    "restricted_p2p_capture": "restricted-p2p.pcapng",
    "restricted_packet_source": "raw-loopback.pcap",
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
LEAKAGE_PAYLOAD_FIELDS = {
    "variant",
    "canaries_injected",
    "canary_commitments",
    "only_secret_fields_changed",
    "capture_complete",
    "finalized_receipt_observed",
    "successful_leg_applications",
    "each_leg_applied_exactly_once",
    "continuous_atomicity_checks",
    "partial_visible_observations",
    "partial_spendable_observations",
    "capture_provenance",
    "artifacts",
    "traffic_counts",
}
LEAKAGE_ARTIFACT_FIELDS = {
    "surface",
    "relative_name",
    "sha256",
    "bytes",
    "source_sha256",
    "source_bytes",
    "source_count",
}
LEAKAGE_BLOCK_WIRE_MAGIC_V1 = b"APSBLK1\0"
LEAKAGE_ARTIFACT_FRAME_DOMAIN_V1 = b"iroha:aps-leakage-artifact:v1\0"
LEAKAGE_RESTRICTED_SOURCE_DOMAIN_V1 = b"APSRAW1\0"

# The reviewed real-process harness lives at
# ``scripts/private_settlement_real_process_harness.py`` and implements genuine
# N=2,3,4,8,16 private/transparent-control benchmarks and the authenticated
# real-process recovery campaign and N=3 secret-only leakage differential under
# this contract. Leakage remains fail-closed when tcpdump, any source artifact,
# or any independently replayed packet/message/record count channel is unavailable.
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

FAULT_CONTROL_EVIDENCE_FILE = "fault-control.jsonl"
FAULT_OBSERVATION_EVIDENCE_FILE = "fault-observations.jsonl"
FAULT_EVIDENCE_MAX_BYTES = 64 * 1024 * 1024
FAULT_CONTINUOUS_OBSERVATION_DOMAIN_V1 = (
    b"iroha:aps-fault-continuous-observation:v1\0"
)
FAULT_CONTINUOUS_OBSERVATION_PHASE_DOMAIN_V1 = (
    b"iroha:aps-fault-continuous-observation-phase:v1\0"
)
FAULT_CONTINUOUS_EXPECTED_UNAVAILABLE_CLASS_V1 = (
    "expected_transport_unavailable"
)
FAULT_CONTINUOUS_MAX_ATTEMPTS_PER_PEER = 20_000
FAULT_STATE_COUNT_FIELDS = {
    "governance",
    "pools",
    "roots",
    "nullifiers",
    "commitments",
    "encrypted_outputs",
    "replay_markers",
    "receipts",
    "abort_markers",
    "staged_pool_heads",
    "staged_nullifiers",
    "staged_output_commitments",
    "replicated_staged_locks",
    "staged_locks",
}
FAULT_STAGED_COUNT_FIELDS = {
    "staged_pool_heads",
    "staged_nullifiers",
    "staged_output_commitments",
    "replicated_staged_locks",
    "staged_locks",
}
FAULT_LEDGER_COUNT_FIELDS = FAULT_STATE_COUNT_FIELDS - FAULT_STAGED_COUNT_FIELDS

LEAKAGE_ACCOUNT_LEFT_I105 = (
    "sorauﾛ1PｺfMﾇﾘｾﾄoﾂﾊﾔH7ZdﾘhﾚmAｸdnｳu1ｱﾄ1ｺﾋuSﾑﾀﾇﾐuHEB5DP"
)
LEAKAGE_ACCOUNT_RIGHT_I105 = (
    "sorauﾛ1NﾑﾅpﾐTm5Yfﾕ3ｦSヰﾏBｶA5ｻﾔｽｱｼDkDｸkVZBｳﾈyｽﾜヰ9NA1NP"
)
LEAKAGE_ASSET_LEFT = "4Zust3cNxfvUrJRuFjSMmNXho9rF"
LEAKAGE_ASSET_RIGHT = "7fnqfbvxnCke21nA2Zy1C3KktDdi"

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


def canonical_iroha_hash_body(value: Any, label: str) -> str:
    """Return the lowercase body of one checksum-valid canonical Hash literal."""

    matched = IROHA_HASH_LITERAL.fullmatch(value) if isinstance(value, str) else None
    if matched is None:
        raise RunnerError(f"{label} is not a canonical Iroha hash literal")
    body = matched.group(1)
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    if value[-4:] != f"{crc:04X}":
        raise RunnerError(f"{label} has an invalid Iroha hash checksum")
    return body.lower()


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


def canonical_participant_visibilities(participants: int) -> list[str]:
    """Return the release profile with one public and remaining restricted legs."""

    if (
        isinstance(participants, bool)
        or not isinstance(participants, int)
        or participants not in PARTICIPANTS
    ):
        raise RunnerError("unsupported participant count for visibility policy")
    return [PUBLIC_PARTICIPANT_VISIBILITY] + [
        RESTRICTED_PARTICIPANT_VISIBILITY
    ] * (participants - 1)


def build_canary_manifest(commit: str) -> dict[str, Any]:
    """Build two deterministic secret sets for the primary differential run."""

    if GIT_OBJECT.fullmatch(commit) is None:
        raise RunnerError("commit must be a full lowercase Git object id")
    seed = hashlib.sha256(f"{PROTOCOL}:{commit}:leakage-canaries".encode()).digest()
    tag_a = seed[:12].hex()
    tag_b = seed[12:24].hex()
    amount_a = (1 << 118) + int.from_bytes(seed[:15], "big") % (1 << 118)
    amount_b = (1 << 118) + int.from_bytes(
        hashlib.sha256(seed).digest()[:15], "big"
    ) % (1 << 118)
    if amount_b == amount_a:
        amount_b += 1
    entries = {
        "account_id": ("text", LEAKAGE_ACCOUNT_LEFT_I105),
        "account_id_variant_b": ("text", LEAKAGE_ACCOUNT_RIGHT_I105),
        "amount": ("integer", amount_a),
        "amount_variant_b": ("integer", amount_b),
        "asset_alias": ("text", f"aps-cbdc-alias-{tag_a}"),
        "asset_alias_variant_b": ("text", f"aps-cbdc-alias-{tag_b}"),
        "asset_id": ("text", LEAKAGE_ASSET_LEFT),
        "asset_id_variant_b": ("text", LEAKAGE_ASSET_RIGHT),
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
        "participant_visibilities": canonical_participant_visibilities(participants),
        "primary_paper_configuration": participants == PRIMARY_PARTICIPANTS,
        "execution": {
            "rayon_worker_threads": RAYON_WORKER_THREADS,
            "validator_worker_threads": VALIDATOR_WORKER_THREADS,
            "cargo_build_jobs": CARGO_BUILD_JOBS,
            "cargo_release_codegen_units": CARGO_RELEASE_CODEGEN_UNITS,
            "cargo_incremental": CARGO_INCREMENTAL,
        },
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
            "traffic_count_channels": list(leakage_audit.REQUIRED_COUNT_CHANNELS),
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
                "traffic_count_channels": list(
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
            "traffic_count_channels",
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
        or requirements["traffic_count_channels"]
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


def read_bound_jsonl_file(path: Path, label: str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Read one stable, bounded, non-empty strict JSONL evidence file."""

    descriptor, metadata = _open_regular_nofollow(path)
    if metadata.st_size <= 0 or metadata.st_size > FAULT_EVIDENCE_MAX_BYTES:
        os.close(descriptor)
        raise RunnerError(f"{label} must be non-empty and at most {FAULT_EVIDENCE_MAX_BYTES} bytes")
    try:
        with os.fdopen(descriptor, "rb") as stream:
            raw = stream.read(FAULT_EVIDENCE_MAX_BYTES + 1)
            final_metadata = os.fstat(stream.fileno())
    except OSError as error:
        raise RunnerError(f"cannot read {label}: {error}") from error
    stable_fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if len(raw) != metadata.st_size or len(raw) > FAULT_EVIDENCE_MAX_BYTES or any(
        getattr(metadata, field) != getattr(final_metadata, field)
        for field in stable_fields
    ):
        raise RunnerError(f"{label} changed while it was read")
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise RunnerError(f"{label} is not UTF-8") from error
    if not text.endswith("\n") or any(not line for line in text.splitlines()):
        raise RunnerError(f"{label} must contain non-empty newline-terminated JSON records")
    records: list[dict[str, Any]] = []
    for index, line in enumerate(text.splitlines()):
        decoded = strict_json_loads(line, f"{label}[{index}]")
        if not isinstance(decoded, dict):
            raise RunnerError(f"{label}[{index}] must be an object")
        records.append(decoded)
    if not records:
        raise RunnerError(f"{label} must contain at least one record")
    return records, {"sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}


def _decode_bound_evidence_json_hex(value: Any, label: str) -> tuple[bytes, dict[str, Any]]:
    """Decode one bounded canonical JSON byte transcript represented as lowercase hex."""

    if (
        not isinstance(value, str)
        or not value
        or len(value) > 2 * 1024 * 1024
        or len(value) % 2
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise RunnerError(f"{label} must be non-empty bounded lowercase hex")
    raw = bytes.fromhex(value)
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise RunnerError(f"{label} is not UTF-8 JSON") from error
    decoded = strict_json_loads(text, label)
    if not isinstance(decoded, dict):
        raise RunnerError(f"{label} must decode to a JSON object")
    if canonical_bytes(decoded) != raw:
        raise RunnerError(f"{label} is not canonical compact JSON")
    return raw, decoded


def _validate_validator_restart_control(
    control: Mapping[str, Any],
    command: Mapping[str, Any],
    acknowledgement: Mapping[str, Any],
    label: str,
) -> None:
    """Bind a validator restart transcript to its peer and PID transition."""

    command = exact_fields(
        command,
        {"format_version", "revision", "operation", "peer_index", "before_pid"},
        f"{label}.command",
    )
    acknowledgement = exact_fields(
        acknowledgement,
        {
            "format_version",
            "revision",
            "command_sha256",
            "operation",
            "peer_index",
            "before_pid",
            "after_pid",
            "health_observed",
        },
        f"{label}.acknowledgement",
    )
    expected_ack_operation = {
        "stop_validator_for_quorum_progress": (
            "validator_restarted_after_quorum_progress"
        ),
        "restart_validator": "restart_validator",
        "recover_crashed_validator": "recover_crashed_validator",
    }.get(command["operation"])
    if (
        command["format_version"] != 1
        or acknowledgement["format_version"] != 1
        or positive_integer(command["revision"], f"{label}.command.revision")
        != acknowledgement["revision"]
        or acknowledgement["command_sha256"] != control["command_sha256"]
        or command["peer_index"] != control["peer_index"]
        or acknowledgement["peer_index"] != control["peer_index"]
        or command["before_pid"] != control["before_pid"]
        or acknowledgement["before_pid"] != control["before_pid"]
        or acknowledgement["after_pid"] != control["after_pid"]
        or expected_ack_operation is None
        or acknowledgement["operation"] != expected_ack_operation
        or acknowledgement["health_observed"] is not True
    ):
        raise RunnerError(f"{label} restart acknowledgement is substituted")


def _validate_coordinator_restart_control(
    control: Mapping[str, Any],
    command: Mapping[str, Any],
    acknowledgement: Mapping[str, Any],
    *,
    participants: int,
    label: str,
) -> None:
    """Bind the restartable helper to complete Prepare/Commit recovery."""

    command = exact_fields(
        command,
        {
            "format_version",
            "revision",
            "operation",
            "committee_endpoints",
            "manifest",
            "authority_catalog",
            "deltas",
            "barrier",
        },
        f"{label}.command",
    )
    acknowledgement = exact_fields(
        acknowledgement,
        {
            "format_version",
            "revision",
            "command_sha256",
            "pid",
            "operation",
            "barrier",
            "commit_certificates",
        },
        f"{label}.acknowledgement",
    )
    endpoints = command["committee_endpoints"]
    if (
        command["format_version"] != 1
        or acknowledgement["format_version"] != 1
        or positive_integer(command["revision"], f"{label}.command.revision")
        != acknowledgement["revision"]
        or command["operation"] != "recover_prepare_commit"
        or acknowledgement["operation"] != command["operation"]
        or acknowledgement["command_sha256"] != control["command_sha256"]
        or acknowledgement["pid"] != control["after_pid"]
        or not isinstance(endpoints, list)
        or len(endpoints) != participants
        or any(
            not isinstance(committee, list)
            or len(committee) != VALIDATORS_PER_DATASPACE
            or any(not isinstance(endpoint, str) or not endpoint for endpoint in committee)
            for committee in endpoints
        )
        or not isinstance(command["manifest"], dict)
        or not isinstance(command["authority_catalog"], list)
        or len(command["authority_catalog"]) != participants
        or not isinstance(command["deltas"], list)
        or len(command["deltas"]) != participants
        or command["barrier"] is not None
        or not isinstance(acknowledgement["barrier"], dict)
        or not isinstance(acknowledgement["commit_certificates"], list)
        or len(acknowledgement["commit_certificates"]) != participants
    ):
        raise RunnerError(f"{label} coordinator recovery acknowledgement is substituted")


def validate_fault_control_records(
    records: Sequence[Mapping[str, Any]],
    *,
    participants: int,
    seed: int,
    run: int,
) -> dict[str, Mapping[str, Any]]:
    """Validate exact durable command/acknowledgement bytes for every fault trial."""

    by_record: dict[str, Mapping[str, Any]] = {}
    observed_control_types: set[str] = set()
    observed_bundle_ids: set[str] = set()
    for index, item in enumerate(records):
        row = exact_fields(
            item,
            {
                "record",
                "bundle_id",
                "participants",
                "seed",
                "run",
                "collection",
                "trial_index",
                "controls",
            },
            f"fault control evidence[{index}]",
        )
        record_id = row["record"]
        if not isinstance(record_id, str) or not re.fullmatch(
            r"[a-z0-9][a-z0-9._:-]{0,127}", record_id
        ):
            raise RunnerError(f"fault control evidence[{index}].record is invalid")
        if record_id in by_record:
            raise RunnerError(f"fault control evidence reuses record {record_id}")
        bundle_id = row["bundle_id"]
        if (
            not isinstance(bundle_id, str)
            or SHA256.fullmatch(bundle_id) is None
            or bundle_id == "0" * 64
        ):
            raise RunnerError(f"fault control evidence[{index}].bundle_id is invalid")
        if bundle_id in observed_bundle_ids:
            raise RunnerError("fault control evidence reuses an APS bundle")
        observed_bundle_ids.add(bundle_id)
        if (
            row["participants"] != participants
            or row["seed"] != seed
            or row["run"] != run
            or row["collection"]
            not in {"loss_trials", "phase_cut_partitions", "crash_recoveries"}
            or isinstance(row["trial_index"], bool)
            or not isinstance(row["trial_index"], int)
            or row["trial_index"] < 0
        ):
            raise RunnerError(f"fault control evidence[{index}] changes its bound job")
        controls = row["controls"]
        if not isinstance(controls, list) or not controls:
            raise RunnerError(f"fault control evidence[{index}].controls must be non-empty")
        bundle_bound = False
        for control_index, item_control in enumerate(controls):
            control = exact_fields(
                item_control,
                {
                    "control_type",
                    "peer_index",
                    "command_sha256",
                    "command_hex",
                    "acknowledgement_sha256",
                    "acknowledgement_hex",
                    "before_pid",
                    "after_pid",
                },
                f"fault control evidence[{index}].controls[{control_index}]",
            )
            control_type = control["control_type"]
            if control_type not in {
                "restricted_da",
                "prepare",
                "commit",
                "consensus_carrier",
                "persistence_cut",
                "validator_restart",
                "global_restart",
                "coordinator_restart",
            }:
                raise RunnerError(
                    f"fault control evidence[{index}].controls[{control_index}] has an unknown type"
                )
            observed_control_types.add(control_type)
            peer_index = control["peer_index"]
            if (
                isinstance(peer_index, bool)
                or not isinstance(peer_index, int)
                or not 0 <= peer_index < (participants + 1) * VALIDATORS_PER_DATASPACE
            ):
                if control_type != "coordinator_restart" or peer_index is not None:
                    raise RunnerError(
                        f"fault control evidence[{index}].controls[{control_index}].peer_index is invalid"
                    )
            command, command_object = _decode_bound_evidence_json_hex(
                control["command_hex"],
                f"fault control evidence[{index}].controls[{control_index}].command_hex",
            )
            acknowledgement, acknowledgement_object = _decode_bound_evidence_json_hex(
                control["acknowledgement_hex"],
                f"fault control evidence[{index}].controls[{control_index}].acknowledgement_hex",
            )
            command_digest = hashlib.sha256(command).hexdigest()
            acknowledgement_digest = hashlib.sha256(acknowledgement).hexdigest()
            if (
                control["command_sha256"] != command_digest
                or control["acknowledgement_sha256"] != acknowledgement_digest
                or SHA256.fullmatch(command_digest) is None
                or SHA256.fullmatch(acknowledgement_digest) is None
            ):
                raise RunnerError(
                    f"fault control evidence[{index}].controls[{control_index}] digest mismatch"
                )
            acknowledged_command = acknowledgement_object.get("command_sha256")
            if acknowledged_command is not None and acknowledged_command != command_digest:
                raise RunnerError(
                    f"fault control evidence[{index}].controls[{control_index}] acknowledgement binds another command"
                )
            if control_type in {"restricted_da", "prepare", "commit"}:
                if command_object.get("bundle_id") != bundle_id:
                    raise RunnerError(
                        f"fault control evidence[{index}].controls[{control_index}] binds another APS bundle"
                    )
                bundle_bound = True
            elif control_type == "persistence_cut":
                if command_object.get("source_id") != bundle_id:
                    raise RunnerError(
                        f"fault control evidence[{index}].controls[{control_index}] cuts another APS bundle"
                    )
                bundle_bound = True
            elif control_type == "coordinator_restart":
                manifest = command_object.get("manifest")
                literal = manifest.get("bundle_id") if isinstance(manifest, dict) else None
                try:
                    literal_body = canonical_iroha_hash_body(
                        literal,
                        f"fault control evidence[{index}].controls[{control_index}].command.manifest.bundle_id",
                    )
                except RunnerError as error:
                    raise RunnerError(
                        f"fault control evidence[{index}].controls[{control_index}] coordinator recovered another APS bundle"
                    ) from error
                if literal_body != bundle_id:
                    raise RunnerError(
                        f"fault control evidence[{index}].controls[{control_index}] coordinator recovered another APS bundle"
                    )
                bundle_bound = True
            if "revision" in command_object and acknowledgement_object.get("revision") != command_object["revision"]:
                raise RunnerError(
                    f"fault control evidence[{index}].controls[{control_index}] revision mismatch"
                )
            before_pid = control["before_pid"]
            after_pid = control["after_pid"]
            for pid, suffix in ((before_pid, "before_pid"), (after_pid, "after_pid")):
                if pid is not None and (
                    isinstance(pid, bool) or not isinstance(pid, int) or pid <= 0
                ):
                    raise RunnerError(
                        f"fault control evidence[{index}].controls[{control_index}].{suffix} is invalid"
                    )
            if control_type.endswith("restart") and (
                before_pid is None or after_pid is None or before_pid == after_pid
            ):
                raise RunnerError(
                    f"fault control evidence[{index}].controls[{control_index}] lacks a genuine PID transition"
                )
            control_label = f"fault control evidence[{index}].controls[{control_index}]"
            if control_type in {"validator_restart", "global_restart"}:
                _validate_validator_restart_control(
                    control,
                    command_object,
                    acknowledgement_object,
                    control_label,
                )
            elif control_type == "coordinator_restart":
                _validate_coordinator_restart_control(
                    control,
                    command_object,
                    acknowledgement_object,
                    participants=participants,
                    label=control_label,
                )
        if not bundle_bound:
            raise RunnerError(
                f"fault control evidence[{index}] has no command bound to its APS bundle"
            )
        by_record[record_id] = row
    required_types = {
        "restricted_da",
        "prepare",
        "commit",
        "consensus_carrier",
        "persistence_cut",
        "validator_restart",
        "global_restart",
        "coordinator_restart",
    }
    if not required_types.issubset(observed_control_types):
        raise RunnerError(
            "fault control evidence is missing genuine control classes: "
            f"{sorted(required_types - observed_control_types)}"
        )
    return by_record


def _decoded_control_pair(
    control: Mapping[str, Any], label: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Return the already hash-bound canonical command and acknowledgement."""

    _command_bytes, command = _decode_bound_evidence_json_hex(
        control["command_hex"], f"{label}.command_hex"
    )
    _ack_bytes, acknowledgement = _decode_bound_evidence_json_hex(
        control["acknowledgement_hex"], f"{label}.acknowledgement_hex"
    )
    return command, acknowledgement


def _validate_route_control_pair(
    control: Mapping[str, Any], label: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    command, acknowledgement = _decoded_control_pair(control, label)
    command = exact_fields(
        command,
        {
            "action",
            "bundle_id",
            "drop_first",
            "format_version",
            "match_limit",
            "phase",
            "revision",
            "seed",
        },
        f"{label}.command",
    )
    acknowledgement = exact_fields(
        acknowledgement,
        {
            "action",
            "bundle_id",
            "command_sha256",
            "dropped",
            "format_version",
            "held",
            "matched",
            "passed",
            "phase",
            "predecessor_command_sha256",
            "released",
            "request_digests",
            "revision",
            "seed",
        },
        f"{label}.acknowledgement",
    )
    if (
        command["format_version"] != 1
        or command["action"] not in {"loss", "hold", "pass"}
        or command["phase"] not in {"restricted_da", "prepare", "commit"}
        or not isinstance(command["bundle_id"], str)
        or SHA256.fullmatch(command["bundle_id"]) is None
        or command["revision"] != acknowledgement["revision"]
        or command["phase"] != acknowledgement["phase"]
        or command["action"] != acknowledgement["action"]
        or command["bundle_id"] != acknowledgement["bundle_id"]
        or command["seed"] != acknowledgement["seed"]
        or acknowledgement["format_version"] != 1
        or acknowledgement["command_sha256"] != control["command_sha256"]
    ):
        raise RunnerError(f"{label} route command acknowledgement is substituted")
    counters = {}
    for field in ("matched", "passed", "dropped", "held", "released"):
        counters[field] = nonnegative_integer(
            acknowledgement[field], f"{label}.acknowledgement.{field}"
        )
    request_digests = acknowledgement["request_digests"]
    if (
        not isinstance(request_digests, list)
        or len(request_digests) != counters["matched"]
        or any(not isinstance(digest, str) or SHA256.fullmatch(digest) is None for digest in request_digests)
        or counters["passed"] + counters["dropped"] + counters["held"]
        != counters["matched"]
        or counters["released"] > counters["held"]
    ):
        raise RunnerError(f"{label} route acknowledgement counters are inconsistent")
    predecessor = acknowledgement["predecessor_command_sha256"]
    if (counters["released"] == 0 and predecessor is not None) or (
        counters["released"] > 0
        and (not isinstance(predecessor, str) or SHA256.fullmatch(predecessor) is None)
    ):
        raise RunnerError(f"{label} route healing predecessor is inconsistent")
    action = command["action"]
    drop_first = nonnegative_integer(command["drop_first"], f"{label}.command.drop_first")
    match_limit = nonnegative_integer(command["match_limit"], f"{label}.command.match_limit")
    if (
        (action == "loss" and not (0 <= drop_first <= match_limit <= 10_000 and match_limit > 0))
        or (action == "hold" and (drop_first, match_limit) != (0, 1))
        or (action == "pass" and (drop_first, match_limit) != (0, 0))
    ):
        raise RunnerError(f"{label} route action bounds are invalid")
    return command, acknowledgement


def _validate_consensus_carrier_control(
    control: Mapping[str, Any], label: str
) -> tuple[str, int]:
    """Validate one exact Sumeragi carrier Hold or drain acknowledgement."""

    command, acknowledgement = _decoded_control_pair(control, label)
    command = exact_fields(
        command,
        {"drain", "queue_capacity", "release", "revision", "rules", "version"},
        f"{label}.command",
    )
    acknowledgement = exact_fields(
        acknowledgement,
        {
            "command_digest",
            "delivered",
            "dropped",
            "drain_fence",
            "draining",
            "fatal",
            "held",
            "held_bytes",
            "in_flight",
            "in_flight_bytes",
            "last_error",
            "overflowed",
            "queue_capacity",
            "rejected_commands",
            "release_pending",
            "retired",
            "revision",
            "rules",
            "version",
        },
        f"{label}.acknowledgement",
    )
    revision = positive_integer(command["revision"], f"{label}.command.revision")
    queue_capacity = positive_integer(
        command["queue_capacity"], f"{label}.command.queue_capacity"
    )
    digest_literal = acknowledgement["command_digest"]
    try:
        digest_body = canonical_iroha_hash_body(
            digest_literal, f"{label}.acknowledgement.command_digest"
        )
    except RunnerError as error:
        raise RunnerError(f"{label} carrier command acknowledgement is substituted") from error
    rules = command["rules"]
    release = command["release"]
    held = acknowledgement["held"]
    delivered = acknowledgement["delivered"]
    retired = acknowledgement["retired"]
    release_pending = acknowledgement["release_pending"]
    if (
        command["version"] != 5
        or acknowledgement["version"] != 5
        or acknowledgement["revision"] != revision
        or digest_body != control["command_sha256"]
        or not isinstance(command["drain"], bool)
        or not isinstance(rules, list)
        or not isinstance(release, list)
        or not isinstance(held, list)
        or not isinstance(delivered, list)
        or not isinstance(retired, list)
        or not isinstance(release_pending, list)
        or acknowledgement["rules"] != rules
        or acknowledgement["queue_capacity"] != queue_capacity
        or acknowledgement["fatal"] is not False
        or acknowledgement["last_error"] is not None
        or acknowledgement["draining"] is not False
        or acknowledgement["in_flight"] is not None
        or acknowledgement["in_flight_bytes"] != 0
        or release_pending
        or nonnegative_integer(
            acknowledgement["dropped"], f"{label}.acknowledgement.dropped"
        )
        != 0
        or nonnegative_integer(
            acknowledgement["overflowed"],
            f"{label}.acknowledgement.overflowed",
        )
        != 0
        or nonnegative_integer(
            acknowledgement["rejected_commands"],
            f"{label}.acknowledgement.rejected_commands",
        )
        != 0
    ):
        raise RunnerError(f"{label} carrier command acknowledgement is substituted")
    held_bytes = nonnegative_integer(
        acknowledgement["held_bytes"], f"{label}.acknowledgement.held_bytes"
    )
    if command["drain"]:
        if (
            queue_capacity != 512
            or rules
            or release
            or held
            or held_bytes != 0
            or acknowledgement["drain_fence"] != revision
            or not delivered + retired
        ):
            raise RunnerError(f"{label} does not prove a completed carrier drain")
        return "heal", revision
    if (
        queue_capacity != 256
        or release
        or not rules
        or not held
        or held_bytes <= 0
        or acknowledgement["drain_fence"] is not None
        or delivered
        or retired
        or any(
            not isinstance(rule, dict)
            or rule.get("action") != "hold"
            or rule.get("kind") != "proposal"
            for rule in rules
        )
    ):
        raise RunnerError(f"{label} does not prove an active carrier Hold")
    return "hold", revision


def validate_fault_trial_control_semantics(
    control_row: Mapping[str, Any],
    *,
    collection: str,
    trial: Mapping[str, Any],
    label: str,
) -> None:
    """Bind each trial declaration to the exact authenticated control semantics."""

    controls = control_row["controls"]
    route_controls: list[tuple[Mapping[str, Any], dict[str, Any], dict[str, Any]]] = []
    for index, control in enumerate(controls):
        if control["control_type"] in {"restricted_da", "prepare", "commit"}:
            command, acknowledgement = _validate_route_control_pair(
                control, f"{label}.controls[{index}]"
            )
            route_controls.append((control, command, acknowledgement))

    if collection == "loss_trials":
        phase = trial["phase"]
        matching = [
            (control, command, ack)
            for control, command, ack in route_controls
            if control["control_type"] == phase and command["phase"] == phase
        ]
        loss = [(command, ack) for _control, command, ack in matching if command["action"] == "loss"]
        healing = [ack for _control, command, ack in matching if command["action"] == "pass"]
        matched = sum(ack["matched"] for _command, ack in loss)
        dropped = sum(ack["dropped"] for _command, ack in loss)
        if (
            len(controls) != 2
            or len(route_controls) != 2
            or len(loss) != 1
            or len(healing) != 1
            or matched == 0
            or dropped * 100 != matched * trial["loss_percent"]
            or any(ack["matched"] == 0 for ack in healing)
            or any(
                control["control_type"] in {"restricted_da", "prepare", "commit"}
                and control["control_type"] != phase
                for control in controls
            )
        ):
            raise RunnerError(f"{label} does not prove its exact route loss and healing")
        return

    if collection == "phase_cut_partitions":
        cut_to_phase = {
            "da_before_availability_qc": "restricted_da",
            "prepare_before_complete_barrier": "prepare",
            "commit_before_complete_barrier": "commit",
        }
        cut = trial["cut"]
        if cut == "carrier_before_global_finality":
            carrier_controls = [
                control
                for control in controls
                if control["control_type"] == "consensus_carrier"
            ]
            validator_restarts = [
                control
                for control in controls
                if control["control_type"] == "validator_restart"
            ]
            global_restarts = [
                control
                for control in controls
                if control["control_type"] == "global_restart"
            ]
            coordinator_restarts = [
                control
                for control in controls
                if control["control_type"] == "coordinator_restart"
            ]
            participants = control_row["participants"]
            participant_restart_coverage = all(
                sum(
                    1
                    for control in validator_restarts
                    if 4 + 4 * dataspace_ordinal
                    <= control["peer_index"]
                    <= 7 + 4 * dataspace_ordinal
                )
                == 1
                for dataspace_ordinal in range(participants)
            )
            carrier_actions: dict[int, list[tuple[str, int]]] = {
                peer_index: [] for peer_index in range(VALIDATORS_PER_DATASPACE)
            }
            for control_index, control in enumerate(controls):
                if control["control_type"] != "consensus_carrier":
                    continue
                peer_index = control["peer_index"]
                if peer_index not in carrier_actions:
                    raise RunnerError(
                        f"{label} controls a carrier outside the global lane"
                    )
                carrier_actions[peer_index].append(
                    _validate_consensus_carrier_control(
                        control, f"{label}.controls[{control_index}]"
                    )
                )
            carrier_peer_coverage = all(
                len(actions) == 2
                and {action for action, _revision in actions} == {"hold", "heal"}
                and next(
                    revision for action, revision in actions if action == "hold"
                )
                < next(revision for action, revision in actions if action == "heal")
                for actions in carrier_actions.values()
            )
            restart_controls = [
                *validator_restarts,
                *global_restarts,
                *coordinator_restarts,
            ]
            pid_transitions = {
                (control["before_pid"], control["after_pid"])
                for control in restart_controls
            }
            before_pids = {control["before_pid"] for control in restart_controls}
            after_pids = {control["after_pid"] for control in restart_controls}
            if (
                route_controls
                or len(controls)
                != 2 * VALIDATORS_PER_DATASPACE + participants + 2
                or
                len(carrier_controls) != 2 * VALIDATORS_PER_DATASPACE
                or not carrier_peer_coverage
                or len(validator_restarts) != participants
                or not participant_restart_coverage
                or len(global_restarts) != 1
                or global_restarts[0]["peer_index"] not in range(VALIDATORS_PER_DATASPACE)
                or len(coordinator_restarts) != 1
                or coordinator_restarts[0]["peer_index"] is not None
                or len(pid_transitions) != len(restart_controls)
                or len(before_pids) != len(restart_controls)
                or len(after_pids) != len(restart_controls)
            ):
                raise RunnerError(
                    f"{label} does not prove the exact carrier control/restart topology"
                )
            return
        phase = cut_to_phase.get(cut)
        holds = [
            (control, command, ack)
            for control, command, ack in route_controls
            if control["control_type"] == phase
            and command["phase"] == phase
            and command["action"] == "hold"
        ]
        passes = [
            (control, command, ack)
            for control, command, ack in route_controls
            if control["control_type"] == phase
            and command["phase"] == phase
            and command["action"] == "pass"
        ]
        hold_digests = {control["command_sha256"] for control, _command, ack in holds if ack["held"] > 0}
        released = sum(ack["released"] for _control, _command, ack in passes)
        if (
            phase is None
            or len(controls) != 2
            or len(route_controls) != 2
            or len(holds) != 1
            or len(passes) != 1
            or len(hold_digests) != 1
            or released == 0
            or any(ack["predecessor_command_sha256"] not in hold_digests for _control, _command, ack in passes)
        ):
            raise RunnerError(f"{label} does not prove an acknowledged Hold-to-Pass phase cut")
        return

    boundary_to_phase = {
        "sidecar_fsync": "after_private_settlement_sidecar_fsync",
        "staged_delta_fsync": "after_private_settlement_staged_delta_fsync",
        "prepare_qc": "after_private_settlement_prepare_qc_fsync",
        "prepare_registration_kura_append": "after_private_settlement_kura_append",
        "prepare_registration_wsv_application": "after_private_settlement_wsv_application",
        "commit_qc": "after_private_settlement_commit_qc_fsync",
        "finalization_kura_append": "after_private_settlement_kura_append",
        "finalization_wsv_application": "after_private_settlement_wsv_application",
        "receipt_publication": "after_private_settlement_receipt_publication",
    }
    expected_phase = boundary_to_phase.get(trial["boundary"])
    cuts = []
    restarts = []
    for index, control in enumerate(controls):
        if control["control_type"] == "persistence_cut":
            command, acknowledgement = _decoded_control_pair(control, f"{label}.controls[{index}]")
            if command != acknowledgement or command.get("phase") != expected_phase:
                raise RunnerError(f"{label} persistence cut acknowledgement is substituted")
            cuts.append(control)
        if control["control_type"].endswith("restart"):
            restarts.append(control)
    boundary = trial["boundary"]
    expected_global_target = boundary in {
        "prepare_registration_kura_append",
        "prepare_registration_wsv_application",
        "finalization_kura_append",
        "finalization_wsv_application",
    }
    expected_restart_type = (
        "global_restart" if expected_global_target else "validator_restart"
    )
    if (
        expected_phase is None
        or len(controls) != 2
        or len(cuts) != 1
        or len(restarts) != 1
        or restarts[0]["control_type"] != expected_restart_type
        or restarts[0]["peer_index"] != cuts[0]["peer_index"]
        or (
            expected_global_target
            and cuts[0]["peer_index"] not in range(VALIDATORS_PER_DATASPACE)
        )
        or (
            not expected_global_target
            and not VALIDATORS_PER_DATASPACE
            <= cuts[0]["peer_index"]
            < (control_row["participants"] + 1) * VALIDATORS_PER_DATASPACE
        )
        or restarts[0]["before_pid"] == restarts[0]["after_pid"]
    ):
        raise RunnerError(f"{label} does not prove its persistence cut and process recovery")


def _validate_fault_state_response(
    item: Any,
    *,
    label: str,
    expected_peer_index: int,
) -> tuple[str, str, str, tuple[tuple[str, int], ...]]:
    observation = exact_fields(
        item,
        {
            "peer_index",
            "response_sha256",
            "response_hex",
            "height",
            "commitment",
            "ledger_commitment",
            "replicated_staged_lock_commitment",
            "staged_lock_commitment",
            "counts",
        },
        label,
    )
    if observation["peer_index"] != expected_peer_index:
        raise RunnerError(f"{label}.peer_index is not canonical")
    raw, decoded = _decode_bound_evidence_json_hex(observation["response_hex"], f"{label}.response_hex")
    if observation["response_sha256"] != hashlib.sha256(raw).hexdigest():
        raise RunnerError(f"{label}.response_sha256 does not bind response_hex")
    decoded = exact_fields(
        decoded,
        {
            "format_version",
            "height",
            "commitment",
            "ledger_commitment",
            "replicated_staged_lock_commitment",
            "staged_lock_commitment",
            "counts",
        },
        f"{label}.decoded_response",
    )
    if decoded["format_version"] != 1:
        raise RunnerError(f"{label} has an unsupported state-evidence version")
    for field in (
        "commitment",
        "ledger_commitment",
        "replicated_staged_lock_commitment",
        "staged_lock_commitment",
    ):
        digest = observation[field]
        canonical_iroha_hash_body(digest, f"{label}.{field}")
        if decoded[field] != digest:
            raise RunnerError(f"{label}.{field} is invalid or substituted")
    if (
        isinstance(observation["height"], bool)
        or not isinstance(observation["height"], int)
        or observation["height"] <= 0
        or decoded["height"] != observation["height"]
    ):
        raise RunnerError(f"{label}.height is invalid or substituted")
    counts = exact_fields(observation["counts"], FAULT_STATE_COUNT_FIELDS, f"{label}.counts")
    decoded_counts = exact_fields(
        decoded["counts"], FAULT_STATE_COUNT_FIELDS, f"{label}.decoded_response.counts"
    )
    normalized_counts = tuple(
        (field, nonnegative_integer(counts[field], f"{label}.counts.{field}"))
        for field in sorted(FAULT_STATE_COUNT_FIELDS)
    )
    normalized_by_name = dict(normalized_counts)
    staged_pool_heads = normalized_by_name["staged_pool_heads"]
    staged_nullifiers = normalized_by_name["staged_nullifiers"]
    staged_outputs = normalized_by_name["staged_output_commitments"]
    staged_total = normalized_by_name["staged_locks"]
    if (
        staged_nullifiers != staged_pool_heads * 2
        or staged_outputs != staged_pool_heads * 3
        or staged_total != staged_pool_heads + staged_nullifiers + staged_outputs
    ):
        raise RunnerError(f"{label}.counts has an impossible local staged-lock shape")
    replicated_total = normalized_by_name["replicated_staged_locks"]
    if replicated_total != 0 and (
        replicated_total <= 1
        or (replicated_total - 1) % 9 != 0
        or not 2 <= (replicated_total - 1) // 9 <= 255
    ):
        raise RunnerError(f"{label}.counts has an impossible replicated staged-lock shape")
    if decoded_counts != counts:
        raise RunnerError(f"{label}.counts do not bind response_hex")
    return (
        observation["ledger_commitment"],
        observation["replicated_staged_lock_commitment"],
        observation["staged_lock_commitment"],
        normalized_counts,
    )


def _fault_observation_phase_contract(
    control_row: Mapping[str, Any],
    *,
    collection: str,
    label: str,
) -> list[tuple[str, frozenset[int], bool, tuple[str, ...]]]:
    """Derive the exact observation contract from canonical authenticated controls."""

    controls = control_row["controls"]
    empty = frozenset()

    def command_binding(control: Mapping[str, Any]) -> str:
        return f"command:{control['command_sha256']}"

    def acknowledgement_binding(control: Mapping[str, Any]) -> str:
        return f"acknowledgement:{control['acknowledgement_sha256']}"

    if collection == "loss_trials":
        loss_count = len(fault_report.REQUIRED_LOSS_PERCENTAGES)
        trial_index = control_row["trial_index"]
        if not 0 <= trial_index < len(fault_report.REQUIRED_LOSS_PHASES) * loss_count:
            raise RunnerError(f"{label} has a non-canonical route-loss trial identity")
        phase = fault_report.REQUIRED_LOSS_PHASES[trial_index // loss_count]
        expected_percentage = fault_report.REQUIRED_LOSS_PERCENTAGES[
            trial_index % loss_count
        ]
        if len(controls) != 2:
            raise RunnerError(f"{label} has a non-canonical route-loss control set")
        parsed = [
            _validate_route_control_pair(control, f"{label}.controls[{index}]")
            for index, control in enumerate(controls)
        ]
        loss_command, loss_ack = parsed[0]
        pass_command, pass_ack = parsed[1]
        if (
            any(control["control_type"] != phase for control in controls)
            or [loss_command["action"], pass_command["action"]] != ["loss", "pass"]
            or any(command["phase"] != phase for command, _ack in parsed)
            or any(command["seed"] != control_row["seed"] for command, _ack in parsed)
            or loss_ack["matched"] == 0
            or loss_ack["dropped"] * 100
            != loss_ack["matched"] * expected_percentage
            or pass_ack["matched"] == 0
            or pass_ack["passed"] != pass_ack["matched"]
            or controls[0]["peer_index"] != VALIDATORS_PER_DATASPACE
            or controls[1]["peer_index"] != controls[0]["peer_index"]
            or loss_command["revision"] >= pass_command["revision"]
        ):
            raise RunnerError(f"{label} has a non-canonical route-loss control set")
        return [
            ("preflight", empty, False, ()),
            (
                f"{phase}_loss",
                empty,
                False,
                (acknowledgement_binding(controls[0]),),
            ),
            (
                "post_recovery",
                empty,
                True,
                (acknowledgement_binding(controls[1]),),
            ),
            ("terminal", empty, True, ()),
        ]

    trial_index = control_row["trial_index"]
    if collection == "phase_cut_partitions" and trial_index in range(3):
        phase = fault_report.REQUIRED_LOSS_PHASES[trial_index]
        if len(controls) != 2:
            raise RunnerError(f"{label} has a non-canonical route-Hold control set")
        parsed = [
            _validate_route_control_pair(control, f"{label}.controls[{index}]")
            for index, control in enumerate(controls)
        ]
        hold_command, hold_ack = parsed[0]
        pass_command, pass_ack = parsed[1]
        if (
            any(control["control_type"] != phase for control in controls)
            or [hold_command["action"], pass_command["action"]] != ["hold", "pass"]
            or any(command["phase"] != phase for command, _ack in parsed)
            or any(command["seed"] != control_row["seed"] for command, _ack in parsed)
            or hold_ack["held"] == 0
            or pass_ack["released"] == 0
            or pass_ack["predecessor_command_sha256"]
            != controls[0]["command_sha256"]
            or controls[0]["peer_index"] != VALIDATORS_PER_DATASPACE
            or controls[1]["peer_index"] != controls[0]["peer_index"]
            or hold_command["revision"] >= pass_command["revision"]
        ):
            raise RunnerError(f"{label} has a non-canonical route-Hold control set")
        return [
            ("preflight", empty, False, ()),
            (
                f"{phase}_hold",
                empty,
                False,
                (acknowledgement_binding(controls[0]),),
            ),
            (
                "post_recovery",
                empty,
                True,
                (acknowledgement_binding(controls[1]),),
            ),
            ("terminal", empty, True, ()),
        ]

    if collection == "phase_cut_partitions" and trial_index == 3:
        participants = control_row["participants"]
        expected_types = (
            ["validator_restart"] * participants
            + ["global_restart", "coordinator_restart"]
            + ["consensus_carrier"] * (2 * VALIDATORS_PER_DATASPACE)
        )
        if (
            len(controls) != len(expected_types)
            or [control["control_type"] for control in controls] != expected_types
        ):
            raise RunnerError(f"{label} has a non-canonical carrier control allowlist")
        committee_controls = controls[:participants]
        committee_targets = tuple(
            (dataspace_ordinal + 1) * VALIDATORS_PER_DATASPACE
            for dataspace_ordinal in range(participants)
        )
        for control_index, (control, expected_peer) in enumerate(
            zip(committee_controls, committee_targets, strict=True)
        ):
            command, _ack = _decoded_control_pair(
                control, f"{label}.controls[{control_index}]"
            )
            if (
                control["peer_index"] != expected_peer
                or command.get("operation") != "stop_validator_for_quorum_progress"
            ):
                raise RunnerError(
                    f"{label} has a non-canonical committee-restart control"
                )
        global_control = controls[participants]
        global_command, _global_ack = _decoded_control_pair(
            global_control, f"{label}.controls[{participants}]"
        )
        if (
            global_control["peer_index"] != 0
            or global_command.get("operation") != "restart_validator"
            or controls[participants + 1]["peer_index"] is not None
        ):
            raise RunnerError(f"{label} has a non-canonical global recovery topology")
        carrier_controls = controls[participants + 2 :]
        carrier_actions = [
            _validate_consensus_carrier_control(
                control, f"{label}.controls[{participants + 2 + index}]"
            )
            for index, control in enumerate(carrier_controls)
        ]
        expected_carrier_sequence = [
            *(('hold', peer_index) for peer_index in range(VALIDATORS_PER_DATASPACE)),
            *(('heal', peer_index) for peer_index in range(VALIDATORS_PER_DATASPACE)),
        ]
        actual_carrier_sequence = [
            (action, control["peer_index"])
            for control, (action, _revision) in zip(
                carrier_controls, carrier_actions, strict=True
            )
        ]
        if actual_carrier_sequence != expected_carrier_sequence:
            raise RunnerError(f"{label} has a non-canonical carrier control sequence")
        hold_controls = carrier_controls[:VALIDATORS_PER_DATASPACE]
        healing_controls = carrier_controls[VALIDATORS_PER_DATASPACE:]
        return [
            ("preflight", empty, False, ()),
            (
                "committee_unavailable",
                frozenset(committee_targets),
                False,
                tuple(command_binding(control) for control in committee_controls),
            ),
            (
                "committee_recovery",
                empty,
                False,
                tuple(
                    acknowledgement_binding(control)
                    for control in committee_controls
                ),
            ),
            (
                "global_restart",
                frozenset({0}),
                False,
                (command_binding(global_control),),
            ),
            (
                "global_recovery",
                empty,
                False,
                (acknowledgement_binding(global_control),),
            ),
            (
                "consensus_carrier_hold",
                empty,
                False,
                tuple(
                    acknowledgement_binding(control) for control in hold_controls
                ),
            ),
            (
                "post_recovery",
                empty,
                True,
                tuple(
                    acknowledgement_binding(control) for control in healing_controls
                ),
            ),
            ("terminal", empty, True, ()),
        ]

    if collection == "crash_recoveries":
        if not 0 <= trial_index < len(fault_report.REQUIRED_CRASH_BOUNDARIES):
            raise RunnerError(f"{label} has a non-canonical crash trial identity")
        expected_phase = {
            "sidecar_fsync": "after_private_settlement_sidecar_fsync",
            "staged_delta_fsync": "after_private_settlement_staged_delta_fsync",
            "prepare_qc": "after_private_settlement_prepare_qc_fsync",
            "prepare_registration_kura_append": "after_private_settlement_kura_append",
            "prepare_registration_wsv_application": "after_private_settlement_wsv_application",
            "commit_qc": "after_private_settlement_commit_qc_fsync",
            "finalization_kura_append": "after_private_settlement_kura_append",
            "finalization_wsv_application": "after_private_settlement_wsv_application",
            "receipt_publication": "after_private_settlement_receipt_publication",
        }[fault_report.REQUIRED_CRASH_BOUNDARIES[trial_index]]
        if len(controls) != 2 or controls[0]["control_type"] != "persistence_cut":
            raise RunnerError(f"{label} has a non-canonical persistence control set")
        cut_command, cut_ack = _decoded_control_pair(controls[0], f"{label}.controls[0]")
        restart_command, _restart_ack = _decoded_control_pair(
            controls[1], f"{label}.controls[1]"
        )
        expected_global_target = trial_index in {3, 4, 6, 7}
        expected_restart_type = (
            "global_restart" if expected_global_target else "validator_restart"
        )
        cut_target = controls[0]["peer_index"]
        if (
            cut_command != cut_ack
            or cut_command.get("phase") != expected_phase
            or controls[1]["control_type"] != expected_restart_type
            or controls[1]["peer_index"] != cut_target
            or restart_command.get("operation") != "recover_crashed_validator"
            or (expected_global_target and cut_target != 0)
            or (not expected_global_target and cut_target != VALIDATORS_PER_DATASPACE)
        ):
            raise RunnerError(f"{label} has a non-canonical persistence control set")
        finalization_cut = trial_index in {6, 7, 8}
        expected_finalized = trial_index in {3, 4, 6, 7, 8}
        return [
            ("preflight", empty, False, ()),
            (
                "persistence_cut",
                frozenset({cut_target}),
                finalization_cut,
                (acknowledgement_binding(controls[0]),),
            ),
            (
                "post_recovery",
                empty,
                expected_finalized,
                (acknowledgement_binding(controls[1]),),
            ),
            ("terminal", empty, expected_finalized, ()),
        ]

    raise RunnerError(f"{label} has no continuous-observation phase contract")


def _fault_attempt_response(
    evidence: Any,
    *,
    label: str,
    peer_index: int,
) -> tuple[str, tuple[str, str, str, tuple[tuple[str, int], ...]]]:
    """Decode one canonical public state response carried by an attempt run."""

    raw, decoded = _decode_bound_evidence_json_hex(evidence, label)
    response_sha256 = hashlib.sha256(raw).hexdigest()
    observation = {
        "peer_index": peer_index,
        "response_sha256": response_sha256,
        "response_hex": evidence,
        "height": decoded.get("height"),
        "commitment": decoded.get("commitment"),
        "ledger_commitment": decoded.get("ledger_commitment"),
        "replicated_staged_lock_commitment": decoded.get(
            "replicated_staged_lock_commitment"
        ),
        "staged_lock_commitment": decoded.get("staged_lock_commitment"),
        "counts": decoded.get("counts"),
    }
    return response_sha256, _validate_fault_state_response(
        observation,
        label=label,
        expected_peer_index=peer_index,
    )


def _fault_ledger_attempt_identity(
    identity: tuple[str, str, str, tuple[tuple[str, int], ...]],
) -> tuple[str, tuple[tuple[str, int], ...]]:
    """Project a state identity onto financial APS state, excluding staged locks."""

    ledger, _replicated_staged, _staged, counts = identity
    return (
        ledger,
        tuple((field, count) for field, count in counts if field in FAULT_LEDGER_COUNT_FIELDS),
    )


def validate_fault_observation_records(
    records: Sequence[Mapping[str, Any]],
    *,
    participants: int,
    seed: int,
    run: int,
    control_by_record: Mapping[str, Mapping[str, Any]],
) -> tuple[dict[str, Mapping[str, Any]], int]:
    """Validate snapshots plus control-bound continuous coverage for every peer."""

    by_record: dict[str, Mapping[str, Any]] = {}
    observed_bundle_ids: set[str] = set()
    total_continuous_checks = 0
    peer_count = (participants + 1) * VALIDATORS_PER_DATASPACE
    for index, item in enumerate(records):
        row = exact_fields(
            item,
            {
                "record",
                "bundle_id",
                "participants",
                "seed",
                "run",
                "collection",
                "trial_index",
                "expected_after_state",
                "continuous_checks",
                "continuous_observations",
                "partial_visibility_observed",
                "partial_spendable_observations",
                "snapshots",
            },
            f"fault observation evidence[{index}]",
        )
        record_id = row["record"]
        if (
            not isinstance(record_id, str)
            or re.fullmatch(r"[a-z0-9][a-z0-9._:-]{0,127}", record_id) is None
            or record_id in by_record
        ):
            raise RunnerError(f"fault observation evidence[{index}].record is invalid or reused")
        bundle_id = row["bundle_id"]
        if (
            not isinstance(bundle_id, str)
            or SHA256.fullmatch(bundle_id) is None
            or bundle_id == "0" * 64
        ):
            raise RunnerError(f"fault observation evidence[{index}].bundle_id is invalid")
        if bundle_id in observed_bundle_ids:
            raise RunnerError("fault observation evidence reuses an APS bundle")
        observed_bundle_ids.add(bundle_id)
        if (
            row["participants"] != participants
            or row["seed"] != seed
            or row["run"] != run
            or row["collection"]
            not in {"loss_trials", "phase_cut_partitions", "crash_recoveries"}
            or isinstance(row["trial_index"], bool)
            or not isinstance(row["trial_index"], int)
            or row["trial_index"] < 0
        ):
            raise RunnerError(f"fault observation evidence[{index}] changes its bound job")
        control_row = control_by_record.get(record_id)
        if (
            control_row is None
            or control_row["bundle_id"] != bundle_id
            or control_row["participants"] != participants
            or control_row["seed"] != seed
            or control_row["run"] != run
            or control_row["collection"] != row["collection"]
            or control_row["trial_index"] != row["trial_index"]
        ):
            raise RunnerError(
                f"fault observation evidence[{index}] has no exact authenticated control record"
            )
        expected_after_state = row["expected_after_state"]
        if expected_after_state not in {"reverted", "finalized"}:
            raise RunnerError(
                f"fault observation evidence[{index}].expected_after_state is invalid"
            )
        requires_full_prepare_locks = row["collection"] != "crash_recoveries"
        if row["collection"] == "crash_recoveries":
            pre_application = {
                "sidecar_fsync",
                "staged_delta_fsync",
                "prepare_qc",
                "commit_qc",
            }
            post_carrier = {
                "prepare_registration_kura_append",
                "prepare_registration_wsv_application",
                "finalization_kura_append",
                "finalization_wsv_application",
                "receipt_publication",
            }
            # The boundary is bound to the payload below. Here the ordered trial
            # index already has the canonical crash-boundary meaning.
            if row["trial_index"] >= len(fault_report.REQUIRED_CRASH_BOUNDARIES):
                raise RunnerError(
                    f"fault observation evidence[{index}].trial_index is out of range"
                )
            boundary = fault_report.REQUIRED_CRASH_BOUNDARIES[row["trial_index"]]
            requires_full_prepare_locks = boundary not in {
                "sidecar_fsync",
                "staged_delta_fsync",
                "prepare_qc",
            }
            required_after_state = (
                "reverted" if boundary in pre_application else "finalized"
            )
            if boundary not in pre_application | post_carrier or expected_after_state != required_after_state:
                raise RunnerError(
                    f"fault observation evidence[{index}] misclassifies crash recovery outcome"
                )
        checks = positive_integer(
            row["continuous_checks"], f"fault observation evidence[{index}].continuous_checks"
        )
        if checks < peer_count:
            raise RunnerError(
                f"fault observation evidence[{index}] did not continuously sample every validator"
            )
        total_continuous_checks += checks
        if row["partial_visibility_observed"] is not False:
            raise RunnerError(f"fault observation evidence[{index}] observed partial visibility")
        if row["partial_spendable_observations"] != 0:
            raise RunnerError(f"fault observation evidence[{index}] observed partial spendability")
        snapshots = row["snapshots"]
        if not isinstance(snapshots, list) or [snapshot.get("label") if isinstance(snapshot, dict) else None for snapshot in snapshots] != [
            "before",
            "nonfinalized",
            "after",
        ]:
            raise RunnerError(
                f"fault observation evidence[{index}] requires before/nonfinalized/after snapshots"
            )
        state_vectors: dict[str, list[tuple[str, str, tuple[tuple[str, int], ...]]]] = {}
        for snapshot_index, snapshot_item in enumerate(snapshots):
            snapshot = exact_fields(
                snapshot_item,
                {"label", "validators"},
                f"fault observation evidence[{index}].snapshots[{snapshot_index}]",
            )
            validators = snapshot["validators"]
            if not isinstance(validators, list) or len(validators) != peer_count:
                raise RunnerError(
                    f"fault observation evidence[{index}].snapshots[{snapshot_index}] must cover every validator"
                )
            state_vectors[snapshot["label"]] = [
                _validate_fault_state_response(
                    validator,
                    label=(
                        f"fault observation evidence[{index}].snapshots[{snapshot_index}]"
                        f".validators[{peer_index}]"
                    ),
                    expected_peer_index=peer_index,
                )
                for peer_index, validator in enumerate(validators)
            ]
        before = state_vectors["before"]
        nonfinalized = state_vectors["nonfinalized"]
        after = state_vectors["after"]
        if len(set(before)) != 1:
            raise RunnerError(
                f"fault observation evidence[{index}] lacks a coherent before state"
            )
        before_ledger = {
            (
                ledger,
                tuple((field, count) for field, count in counts if field in FAULT_LEDGER_COUNT_FIELDS),
            )
            for ledger, _replicated_staged, _staged, counts in before
        }
        nonfinalized_ledger = {
            (
                ledger,
                tuple((field, count) for field, count in counts if field in FAULT_LEDGER_COUNT_FIELDS),
            )
            for ledger, _replicated_staged, _staged, counts in nonfinalized
        }
        if len(before_ledger) != 1 or nonfinalized_ledger != before_ledger:
            raise RunnerError(
                f"fault observation evidence[{index}] changed a financial APS map before finality"
            )
        if requires_full_prepare_locks:
            before_counts = dict(before[0][3])
            lock_fields = {
                "staged_pool_heads",
                "staged_nullifiers",
                "staged_output_commitments",
                "replicated_staged_locks",
                "staged_locks",
            }
            if any(before_counts[field] != 0 for field in lock_fields):
                raise RunnerError(
                    f"fault observation evidence[{index}] Prepare-lock baseline is not empty"
                )
            expected_replicated_count = 1 + participants * 9
            replicated_states = {
                (replicated_staged, dict(counts)["replicated_staged_locks"])
                for _ledger, replicated_staged, _staged, counts in nonfinalized
            }
            if replicated_states != {
                (nonfinalized[0][1], expected_replicated_count)
            } or nonfinalized[0][1] == before[0][1]:
                raise RunnerError(
                    f"fault observation evidence[{index}] lacks one coherent full replicated Prepare lock"
                )
            committee_commitments: dict[int, str] = {}
            for peer_index, (_ledger, _replicated, local_commitment, counts_tuple) in enumerate(
                nonfinalized
            ):
                counts = dict(counts_tuple)
                if peer_index < VALIDATORS_PER_DATASPACE:
                    if (
                        any(
                            counts[field] != 0
                            for field in {
                                "staged_pool_heads",
                                "staged_nullifiers",
                                "staged_output_commitments",
                                "staged_locks",
                            }
                        )
                        or local_commitment != before[0][2]
                    ):
                        raise RunnerError(
                            f"fault observation evidence[{index}] gives a global validator a local lock"
                        )
                    continue
                if (
                    counts["staged_pool_heads"] != 1
                    or counts["staged_nullifiers"] != 2
                    or counts["staged_output_commitments"] != 3
                    or counts["staged_locks"] != 6
                    or local_commitment == before[0][2]
                ):
                    raise RunnerError(
                        f"fault observation evidence[{index}] lacks one complete local leg lock"
                    )
                committee = (
                    peer_index - VALIDATORS_PER_DATASPACE
                ) // VALIDATORS_PER_DATASPACE
                existing = committee_commitments.setdefault(
                    committee, local_commitment
                )
                if existing != local_commitment:
                    raise RunnerError(
                        f"fault observation evidence[{index}] has divergent committee-local locks"
                    )
            if len(committee_commitments) != participants:
                raise RunnerError(
                    f"fault observation evidence[{index}] omits a committee-local lock"
                )
        if len(set(after)) != 1:
            raise RunnerError(
                f"fault observation evidence[{index}] did not converge every validator"
            )
        before_identity = before[0]
        after_identity = after[0]
        if expected_after_state == "reverted":
            if after_identity != before_identity:
                raise RunnerError(
                    f"fault observation evidence[{index}] did not restore pre-carrier state"
                )
        else:
            (
                before_ledger_commitment,
                before_replicated_staged,
                before_staged,
                before_counts_tuple,
            ) = before_identity
            (
                after_ledger_commitment,
                after_replicated_staged,
                after_staged,
                after_counts_tuple,
            ) = after_identity
            before_counts = dict(before_counts_tuple)
            after_counts = dict(after_counts_tuple)
            unchanged = {
                "governance",
                "pools",
                "abort_markers",
                "staged_pool_heads",
                "staged_nullifiers",
                "staged_output_commitments",
                "replicated_staged_locks",
                "staged_locks",
            }
            expected_deltas = {
                "roots": participants,
                "nullifiers": participants * 2,
                "commitments": participants * 3,
                "encrypted_outputs": participants * 3,
                "replay_markers": 1,
                "receipts": 1,
            }
            if (
                after_ledger_commitment == before_ledger_commitment
                or after_replicated_staged != before_replicated_staged
                or after_staged != before_staged
                or after_counts["replicated_staged_locks"] != 0
                or after_counts["staged_locks"] != 0
                or any(after_counts[field] != before_counts[field] for field in unchanged)
                or any(
                    after_counts[field] != before_counts[field] + delta
                    for field, delta in expected_deltas.items()
                )
            ):
                raise RunnerError(
                    f"fault observation evidence[{index}] is not one complete {participants}-leg finalization"
                )
        phase_contract = _fault_observation_phase_contract(
            control_row,
            collection=row["collection"],
            label=f"fault observation evidence[{index}]",
        )
        continuous_observations = row["continuous_observations"]
        if not isinstance(continuous_observations, list) or len(continuous_observations) != peer_count:
            raise RunnerError(
                f"fault observation evidence[{index}].continuous_observations must cover every validator"
            )
        captured_checks = 0
        for peer_index, summary_item in enumerate(continuous_observations):
            summary = exact_fields(
                summary_item,
                {
                    "peer_index",
                    "check_count",
                    "poll_failure_count",
                    "first_response_sha256",
                    "last_response_sha256",
                    "response_chain_sha256",
                    "baseline_observations",
                    "finalized_observations",
                    "phase_coverage",
                },
                f"fault observation evidence[{index}].continuous_observations[{peer_index}]",
            )
            if summary["peer_index"] != peer_index:
                raise RunnerError(
                    f"fault observation evidence[{index}].continuous_observations is reordered"
                )
            check_count = positive_integer(
                summary["check_count"],
                f"fault observation evidence[{index}].continuous_observations[{peer_index}].check_count",
            )
            if check_count < 3:
                raise RunnerError(
                    f"fault observation evidence[{index}].continuous_observations[{peer_index}] lacks a live polling observation"
                )
            poll_failure_count = nonnegative_integer(
                summary["poll_failure_count"],
                f"fault observation evidence[{index}].continuous_observations[{peer_index}].poll_failure_count",
            )
            baseline_observations = nonnegative_integer(
                summary["baseline_observations"],
                f"fault observation evidence[{index}].continuous_observations[{peer_index}].baseline_observations",
            )
            finalized_observations = nonnegative_integer(
                summary["finalized_observations"],
                f"fault observation evidence[{index}].continuous_observations[{peer_index}].finalized_observations",
            )
            if baseline_observations + finalized_observations != check_count:
                raise RunnerError(
                    f"fault observation evidence[{index}].continuous_observations[{peer_index}] has an unclassified observation"
                )
            if baseline_observations == 0 or (
                expected_after_state == "reverted" and finalized_observations != 0
            ) or (
                expected_after_state == "finalized" and finalized_observations == 0
            ):
                raise RunnerError(
                    f"fault observation evidence[{index}].continuous_observations[{peer_index}] contradicts the trial outcome"
                )
            response_chain = hashlib.sha256()
            response_chain.update(FAULT_CONTINUOUS_OBSERVATION_DOMAIN_V1)
            response_chain.update(bytes.fromhex(bundle_id))
            response_chain.update(struct.pack("<Q", peer_index))
            recomputed_first_response: str | None = None
            recomputed_last_response: str | None = None
            seen_finalized = False
            phase_coverage = summary["phase_coverage"]
            if (
                not isinstance(phase_coverage, list)
                or len(phase_coverage) != len(phase_contract)
            ):
                raise RunnerError(
                    f"fault observation evidence[{index}].continuous_observations[{peer_index}] has incomplete phase coverage"
                )
            captured_phase_successes = 0
            captured_phase_failures = 0
            captured_phase_baseline = 0
            captured_phase_finalized = 0
            for phase_index, (phase_item, contract) in enumerate(
                zip(phase_coverage, phase_contract, strict=True)
            ):
                phase = exact_fields(
                    phase_item,
                    {
                        "phase",
                        "expected_unavailable",
                        "finalization_allowed",
                        "successful_observations",
                        "poll_failures",
                        "baseline_observations",
                        "finalized_observations",
                        "checkpoint_attempt",
                        "checkpoint_control_bindings",
                        "attempt_chain_sha256",
                        "attempts",
                    },
                    (
                        f"fault observation evidence[{index}].continuous_observations"
                        f"[{peer_index}].phase_coverage[{phase_index}]"
                    ),
                )
                (
                    expected_phase,
                    expected_unavailable_peers,
                    finalization_allowed,
                    expected_checkpoint_bindings,
                ) = contract
                expected_unavailable = peer_index in expected_unavailable_peers
                if (
                    phase["phase"] != expected_phase
                    or phase["expected_unavailable"] is not expected_unavailable
                    or phase["finalization_allowed"] is not finalization_allowed
                    or phase["checkpoint_control_bindings"]
                    != list(expected_checkpoint_bindings)
                ):
                    raise RunnerError(
                        f"fault observation evidence[{index}].continuous_observations[{peer_index}] phase coverage contradicts authenticated controls"
                    )
                phase_successes = nonnegative_integer(
                    phase["successful_observations"],
                    (
                        f"fault observation evidence[{index}].continuous_observations"
                        f"[{peer_index}].phase_coverage[{phase_index}].successful_observations"
                    ),
                )
                phase_failures = nonnegative_integer(
                    phase["poll_failures"],
                    (
                        f"fault observation evidence[{index}].continuous_observations"
                        f"[{peer_index}].phase_coverage[{phase_index}].poll_failures"
                    ),
                )
                phase_baseline = nonnegative_integer(
                    phase["baseline_observations"],
                    (
                        f"fault observation evidence[{index}].continuous_observations"
                        f"[{peer_index}].phase_coverage[{phase_index}].baseline_observations"
                    ),
                )
                phase_finalized = nonnegative_integer(
                    phase["finalized_observations"],
                    (
                        f"fault observation evidence[{index}].continuous_observations"
                        f"[{peer_index}].phase_coverage[{phase_index}].finalized_observations"
                    ),
                )
                checkpoint_attempt = nonnegative_integer(
                    phase["checkpoint_attempt"],
                    (
                        f"fault observation evidence[{index}].continuous_observations"
                        f"[{peer_index}].phase_coverage[{phase_index}].checkpoint_attempt"
                    ),
                )
                if expected_phase == "preflight" and phase_successes < 2:
                    raise RunnerError(
                        f"fault observation evidence[{index}].continuous_observations[{peer_index}] lacks a live preflight poll"
                    )
                attempts = phase["attempts"]
                if not isinstance(attempts, list) or not attempts:
                    raise RunnerError(
                        f"fault observation evidence[{index}].continuous_observations[{peer_index}] has no ordered phase attempts"
                    )
                attempt_chain = hashlib.sha256()
                attempt_chain.update(FAULT_CONTINUOUS_OBSERVATION_PHASE_DOMAIN_V1)
                attempt_chain.update(bytes.fromhex(bundle_id))
                attempt_chain.update(struct.pack("<Q", peer_index))
                attempt_chain.update(struct.pack("<Q", phase_index))
                phase_name = expected_phase.encode("ascii")
                attempt_chain.update(struct.pack("<Q", len(phase_name)))
                attempt_chain.update(phase_name)
                attempt_chain.update(bytes((int(expected_unavailable),)))
                attempt_chain.update(bytes((int(finalization_allowed),)))
                logical_attempt = 0
                recomputed_phase_successes = 0
                recomputed_phase_failures = 0
                recomputed_phase_baseline = 0
                recomputed_phase_finalized = 0
                post_checkpoint_successes = 0
                post_checkpoint_failures = 0
                previous_run: tuple[str, str] | None = None
                for run_index, attempt_item in enumerate(attempts):
                    attempt = exact_fields(
                        attempt_item,
                        {"class", "evidence", "repetitions"},
                        (
                            f"fault observation evidence[{index}].continuous_observations"
                            f"[{peer_index}].phase_coverage[{phase_index}].attempts[{run_index}]"
                        ),
                    )
                    attempt_class = attempt["class"]
                    evidence = attempt["evidence"]
                    repetitions = positive_integer(
                        attempt["repetitions"],
                        (
                            f"fault observation evidence[{index}].continuous_observations"
                            f"[{peer_index}].phase_coverage[{phase_index}]"
                            f".attempts[{run_index}].repetitions"
                        ),
                    )
                    if (
                        attempt_class
                        not in {"baseline", "finalized", "expected_unavailable"}
                        or not isinstance(evidence, str)
                        or previous_run == (attempt_class, evidence)
                    ):
                        raise RunnerError(
                            f"fault observation evidence[{index}].continuous_observations[{peer_index}] has a non-canonical attempt stream"
                        )
                    previous_run = (attempt_class, evidence)
                    if (
                        logical_attempt + repetitions
                        > FAULT_CONTINUOUS_MAX_ATTEMPTS_PER_PEER
                    ):
                        raise RunnerError(
                            f"fault observation evidence[{index}].continuous_observations[{peer_index}] exceeds the attempt bound"
                        )
                    response_digest: str | None = None
                    response_identity = None
                    if attempt_class == "expected_unavailable":
                        if (
                            not expected_unavailable
                            or evidence
                            != FAULT_CONTINUOUS_EXPECTED_UNAVAILABLE_CLASS_V1
                        ):
                            raise RunnerError(
                                f"fault observation evidence[{index}].continuous_observations[{peer_index}] has an unallowlisted poll failure"
                            )
                    else:
                        response_digest, response_identity = _fault_attempt_response(
                            evidence,
                            label=(
                                f"fault observation evidence[{index}].continuous_observations"
                                f"[{peer_index}].phase_coverage[{phase_index}]"
                                f".attempts[{run_index}].evidence"
                            ),
                            peer_index=peer_index,
                        )
                        if attempt_class == "baseline":
                            if (
                                seen_finalized
                                or _fault_ledger_attempt_identity(response_identity)
                                != _fault_ledger_attempt_identity(before_identity)
                            ):
                                raise RunnerError(
                                    f"fault observation evidence[{index}].continuous_observations[{peer_index}] finalized state rolled back or was misclassified"
                                )
                        elif (
                            not finalization_allowed
                            or expected_after_state != "finalized"
                            or response_identity != after_identity
                        ):
                            raise RunnerError(
                                f"fault observation evidence[{index}].continuous_observations[{peer_index}] finalized in a disallowed phase or was misclassified"
                            )
                    for _ in range(repetitions):
                        after_checkpoint = logical_attempt >= checkpoint_attempt
                        if attempt_class == "expected_unavailable":
                            recomputed_phase_failures += 1
                            if after_checkpoint:
                                post_checkpoint_failures += 1
                            attempt_chain.update(b"\x00")
                            attempt_chain.update(
                                FAULT_CONTINUOUS_EXPECTED_UNAVAILABLE_CLASS_V1.encode(
                                    "ascii"
                                )
                            )
                        else:
                            assert response_digest is not None
                            recomputed_phase_successes += 1
                            if after_checkpoint:
                                post_checkpoint_successes += 1
                            if attempt_class == "baseline":
                                recomputed_phase_baseline += 1
                                attempt_chain.update(b"\x01")
                            else:
                                recomputed_phase_finalized += 1
                                seen_finalized = True
                                attempt_chain.update(b"\x02")
                            digest_bytes = bytes.fromhex(response_digest)
                            attempt_chain.update(digest_bytes)
                            response_chain.update(digest_bytes)
                            recomputed_first_response = (
                                recomputed_first_response or response_digest
                            )
                            recomputed_last_response = response_digest
                        logical_attempt += 1
                if not 0 <= checkpoint_attempt < logical_attempt:
                    raise RunnerError(
                        f"fault observation evidence[{index}].continuous_observations[{peer_index}] has no post-checkpoint attempt"
                    )
                attempt_chain.update(b"checkpoint\0")
                attempt_chain.update(struct.pack("<Q", checkpoint_attempt))
                attempt_chain.update(b"checkpoint-controls\0")
                attempt_chain.update(
                    struct.pack("<Q", len(expected_checkpoint_bindings))
                )
                for binding in expected_checkpoint_bindings:
                    encoded_binding = binding.encode("ascii")
                    attempt_chain.update(struct.pack("<Q", len(encoded_binding)))
                    attempt_chain.update(encoded_binding)
                if (
                    recomputed_phase_successes != phase_successes
                    or recomputed_phase_failures != phase_failures
                    or recomputed_phase_baseline != phase_baseline
                    or recomputed_phase_finalized != phase_finalized
                    or phase_baseline + phase_finalized != phase_successes
                    or attempt_chain.hexdigest() != phase["attempt_chain_sha256"]
                ):
                    raise RunnerError(
                        f"fault observation evidence[{index}].continuous_observations[{peer_index}] attempt stream does not bind its phase summary"
                    )
                if expected_unavailable:
                    if post_checkpoint_failures == 0:
                        raise RunnerError(
                            f"fault observation evidence[{index}].continuous_observations[{peer_index}] did not observe its authenticated outage after the fault checkpoint"
                        )
                elif post_checkpoint_successes == 0 or phase_failures != 0:
                    raise RunnerError(
                        f"fault observation evidence[{index}].continuous_observations[{peer_index}] did not successfully cover an available phase after the fault checkpoint"
                    )
                captured_phase_successes += phase_successes
                captured_phase_failures += phase_failures
                captured_phase_baseline += phase_baseline
                captured_phase_finalized += phase_finalized
            if (
                captured_phase_successes != check_count
                or captured_phase_failures != poll_failure_count
                or captured_phase_baseline != baseline_observations
                or captured_phase_finalized != finalized_observations
                or captured_phase_successes + captured_phase_failures
                > FAULT_CONTINUOUS_MAX_ATTEMPTS_PER_PEER
            ):
                raise RunnerError(
                    f"fault observation evidence[{index}].continuous_observations[{peer_index}] phase totals do not equal its summary"
                )
            if (
                recomputed_first_response is None
                or recomputed_last_response is None
                or summary["first_response_sha256"] != recomputed_first_response
                or summary["last_response_sha256"] != recomputed_last_response
                or summary["response_chain_sha256"] != response_chain.hexdigest()
                or recomputed_first_response
                != snapshots[0]["validators"][peer_index]["response_sha256"]
                or recomputed_last_response
                != snapshots[2]["validators"][peer_index]["response_sha256"]
            ):
                raise RunnerError(
                    f"fault observation evidence[{index}].continuous_observations[{peer_index}] chain is invalid or not bound to the exemplar endpoints"
                )
            captured_checks += check_count
        if captured_checks != checks:
            raise RunnerError(
                f"fault observation evidence[{index}].continuous_checks does not equal its per-validator summaries"
            )
        by_record[record_id] = row
    return by_record, total_continuous_checks


def materialize_fault_response(
    response: Mapping[str, Any],
    *,
    plan: Mapping[str, Any],
    job: Mapping[str, Any],
    evidence_dir: Path,
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
            "control_transcript_sha256",
            "control_transcript_record",
            "observation_capture_sha256",
            "observation_capture_record",
        },
        "phase_cut_partitions": {
            "cut",
            "control_acknowledged",
            "delayed_delivery",
            "healed",
            "converged",
            "partial_visibility_observed",
            "control_transcript_sha256",
            "control_transcript_record",
            "observation_capture_sha256",
            "observation_capture_record",
        },
        "crash_recoveries": {
            "boundary",
            "process_restarted",
            "durable_state_reconciled",
            "converged",
            "partial_visibility_observed",
            "control_transcript_sha256",
            "control_transcript_record",
            "observation_capture_sha256",
            "observation_capture_record",
        },
    }
    try:
        evidence_entries = list(evidence_dir.iterdir())
    except OSError as error:
        raise RunnerError(f"cannot enumerate fault evidence: {error}") from error
    expected_evidence_names = {
        FAULT_CONTROL_EVIDENCE_FILE,
        FAULT_OBSERVATION_EVIDENCE_FILE,
    }
    if (
        {entry.name for entry in evidence_entries} != expected_evidence_names
        or any(entry.is_symlink() or not entry.is_file() for entry in evidence_entries)
    ):
        raise RunnerError(
            "fault harness must emit exactly the bound control and observation JSONL files"
        )
    control_source = regular_file_under(
        evidence_dir,
        PurePosixPath(FAULT_CONTROL_EVIDENCE_FILE),
        "fault control evidence",
    )
    observation_source = regular_file_under(
        evidence_dir,
        PurePosixPath(FAULT_OBSERVATION_EVIDENCE_FILE),
        "fault observation evidence",
    )
    control_rows, control_binding = read_bound_jsonl_file(
        control_source, "fault control evidence"
    )
    observation_rows, observation_binding = read_bound_jsonl_file(
        observation_source, "fault observation evidence"
    )
    control_by_record = validate_fault_control_records(
        control_rows, participants=participants, seed=seed, run=run
    )
    observation_by_record, total_continuous_checks = validate_fault_observation_records(
        observation_rows,
        participants=participants,
        seed=seed,
        run=run,
        control_by_record=control_by_record,
    )
    prepared: dict[str, list[dict[str, Any]]] = {}
    atomicity = payload["atomicity"]
    if not isinstance(atomicity, dict):
        raise RunnerError("fault payload.atomicity must be an object")
    validate_prepare_qc_normalization(
        payload["prepare_qc_normalization"],
        "fault payload.prepare_qc_normalization",
    )
    observed_bundle_ids: set[str] = set()
    for collection in collections:
        trials = payload[collection]
        if not isinstance(trials, list):
            raise RunnerError(f"fault payload.{collection} must be a list")
        prepared[collection] = []
        for index, item in enumerate(trials):
            trial = exact_fields(
                item, trial_fields[collection], f"fault payload.{collection}[{index}]"
            )
            record_id = f"n{participants}:s{seed}:r{run}:{collection}:{index}"
            if (
                trial["control_transcript_sha256"] != control_binding["sha256"]
                or trial["control_transcript_record"] != record_id
                or trial["observation_capture_sha256"]
                != observation_binding["sha256"]
                or trial["observation_capture_record"] != record_id
            ):
                raise RunnerError(
                    f"fault payload.{collection}[{index}] does not bind the exact harness evidence"
                )
            control_row = control_by_record.get(record_id)
            observation_row = observation_by_record.get(record_id)
            if (
                control_row is None
                or observation_row is None
                or control_row["collection"] != collection
                or observation_row["collection"] != collection
                or control_row["trial_index"] != index
                or observation_row["trial_index"] != index
                or control_row["bundle_id"] != observation_row["bundle_id"]
                or control_row["bundle_id"] in observed_bundle_ids
            ):
                raise RunnerError(
                    f"fault payload.{collection}[{index}] has no unique exact bundle-bound evidence record"
                )
            observed_bundle_ids.add(control_row["bundle_id"])
            validate_fault_trial_control_semantics(
                control_row,
                collection=collection,
                trial=trial,
                label=f"fault payload.{collection}[{index}]",
            )
            if (
                observation_row["partial_visibility_observed"]
                != trial["partial_visibility_observed"]
                or observation_row["partial_spendable_observations"]
                != atomicity.get("partial_spendable_observations")
            ):
                raise RunnerError(
                    f"fault payload.{collection}[{index}] contradicts its state observations"
                )
            prepared[collection].append(dict(trial))
    expected_record_count = sum(len(prepared[collection]) for collection in collections)
    if (
        len(control_by_record) != expected_record_count
        or len(observation_by_record) != expected_record_count
    ):
        raise RunnerError("fault evidence contains missing or undeclared trial records")
    if atomicity.get("continuous_checks") != total_continuous_checks:
        raise RunnerError(
            "fault payload.atomicity.continuous_checks does not equal the captured observations"
        )
    stem = f"n{participants}-s{seed}-r{run}"
    control_path = publication_root / "fault" / "control" / f"{stem}.jsonl"
    capture_path = publication_root / "fault" / "observations" / f"{stem}.jsonl"
    copy_bound_file(control_source, control_path, expected=control_binding)
    copy_bound_file(observation_source, capture_path, expected=observation_binding)
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


def _read_exact_stream(stream: Any, length: int, label: str) -> bytes:
    raw = stream.read(length)
    if len(raw) != length:
        raise RunnerError(f"{label} is truncated")
    return raw


def _derive_leakage_block_count(path: Path) -> int:
    """Replay the bounded release-only block-wire framing without decoding blocks."""

    descriptor, metadata = _open_regular_nofollow(path)
    try:
        with os.fdopen(descriptor, "rb") as stream:
            if _read_exact_stream(stream, len(LEAKAGE_BLOCK_WIRE_MAGIC_V1), "block wire") != LEAKAGE_BLOCK_WIRE_MAGIC_V1:
                raise RunnerError("leakage block-wire capture has the wrong framing domain")
            count = struct.unpack("<I", _read_exact_stream(stream, 4, "block count"))[0]
            if count != 1:
                raise RunnerError("leakage block-wire capture must contain exactly one carrier")
            consumed = len(LEAKAGE_BLOCK_WIRE_MAGIC_V1) + 4
            for index in range(count):
                length = struct.unpack(
                    "<Q", _read_exact_stream(stream, 8, f"block[{index}] length")
                )[0]
                if length == 0 or length > leakage_audit.DEFAULT_MAX_FILE_BYTES:
                    raise RunnerError("leakage block-wire payload length is outside its bound")
                consumed += 8
                remaining = length
                while remaining:
                    chunk = stream.read(min(remaining, 1024 * 1024))
                    if not chunk:
                        raise RunnerError(f"leakage block[{index}] is truncated")
                    remaining -= len(chunk)
                    consumed += len(chunk)
            if stream.read(1):
                raise RunnerError("leakage block-wire capture has trailing bytes")
            final_metadata = os.fstat(stream.fileno())
    finally:
        if descriptor >= 0:
            try:
                os.close(descriptor)
            except OSError:
                pass
    stable = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if consumed != metadata.st_size or any(
        getattr(metadata, field) != getattr(final_metadata, field) for field in stable
    ):
        raise RunnerError("leakage block-wire capture changed during replay")
    return count


def _validate_leakage_digest_derivative(
    path: Path, expected_kind: str
) -> tuple[int, int, list[dict[str, Any]]]:
    """Validate a Kura/merge/checkpoint artifact contains digests, never raw bytes."""

    descriptor, metadata = _open_regular_nofollow(path)
    try:
        with os.fdopen(descriptor, "rb") as stream:
            if _read_exact_stream(
                stream,
                len(LEAKAGE_ARTIFACT_FRAME_DOMAIN_V1),
                "leakage digest derivative",
            ) != LEAKAGE_ARTIFACT_FRAME_DOMAIN_V1:
                raise RunnerError("leakage digest derivative has the wrong domain")
            kind_length = struct.unpack(
                "<H", _read_exact_stream(stream, 2, "leakage derivative kind length")
            )[0]
            if kind_length == 0 or kind_length > 32:
                raise RunnerError("leakage digest derivative kind is outside its bound")
            try:
                kind = _read_exact_stream(
                    stream, kind_length, "leakage derivative kind"
                ).decode("ascii")
            except UnicodeError as error:
                raise RunnerError("leakage digest derivative kind is not ASCII") from error
            if kind != expected_kind:
                raise RunnerError("leakage digest derivative kind was substituted")
            count = struct.unpack(
                "<I", _read_exact_stream(stream, 4, "leakage derivative count")
            )[0]
            if count == 0 or count > 100_000:
                raise RunnerError("leakage digest derivative count is outside its bound")
            total_source_bytes = 0
            rows: list[dict[str, Any]] = []
            for index in range(count):
                ordinal = struct.unpack(
                    "<I",
                    _read_exact_stream(stream, 4, f"leakage derivative[{index}] ordinal"),
                )[0]
                path_digest = _read_exact_stream(
                    stream, 32, f"leakage derivative[{index}] path digest"
                )
                source_bytes = struct.unpack(
                    "<Q",
                    _read_exact_stream(stream, 8, f"leakage derivative[{index}] size"),
                )[0]
                source_digest = _read_exact_stream(
                    stream, 32, f"leakage derivative[{index}] source digest"
                )
                if (
                    ordinal != index
                    or path_digest == bytes(32)
                    or source_bytes == 0
                    or source_digest == bytes(32)
                ):
                    raise RunnerError("leakage digest derivative row is invalid")
                total_source_bytes += source_bytes
                if total_source_bytes > leakage_audit.DEFAULT_MAX_TOTAL_BYTES:
                    raise RunnerError("leakage derivative source total exceeds its bound")
                rows.append(
                    {
                        "path_sha256": path_digest.hex(),
                        "source_bytes": source_bytes,
                        "source_sha256": source_digest.hex(),
                    }
                )
            if stream.read(1):
                raise RunnerError("leakage digest derivative has trailing bytes")
            final_metadata = os.fstat(stream.fileno())
    finally:
        try:
            os.close(descriptor)
        except OSError:
            pass
    stable = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if any(
        getattr(metadata, field) != getattr(final_metadata, field) for field in stable
    ):
        raise RunnerError("leakage digest derivative changed during replay")
    return count, total_source_bytes, rows


def _validate_restricted_leakage_source_archive(
    path: Path,
) -> dict[str, dict[str, Any]]:
    """Replay the retained raw-source archive into per-surface bindings."""

    descriptor, metadata = _open_regular_nofollow(path)
    rows: list[dict[str, Any]] = []
    try:
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            if _read_exact_stream(
                stream,
                len(LEAKAGE_RESTRICTED_SOURCE_DOMAIN_V1),
                "restricted leakage source domain",
            ) != LEAKAGE_RESTRICTED_SOURCE_DOMAIN_V1:
                raise RunnerError("restricted leakage source archive has the wrong domain")
            count = struct.unpack(
                "<I", _read_exact_stream(stream, 4, "restricted source count")
            )[0]
            if count == 0 or count > 100_000:
                raise RunnerError("restricted leakage source count is outside its bound")
            previous: tuple[str, str] | None = None
            total_source_bytes = 0
            for index in range(count):
                ordinal = struct.unpack(
                    "<I",
                    _read_exact_stream(stream, 4, f"restricted source[{index}] ordinal"),
                )[0]
                surface_length = struct.unpack(
                    "<H",
                    _read_exact_stream(
                        stream, 2, f"restricted source[{index}] surface length"
                    ),
                )[0]
                if surface_length == 0 or surface_length > 64:
                    raise RunnerError("restricted leakage source surface is outside its bound")
                try:
                    surface = _read_exact_stream(
                        stream,
                        surface_length,
                        f"restricted source[{index}] surface",
                    ).decode("ascii")
                except UnicodeError as error:
                    raise RunnerError("restricted leakage source surface is not ASCII") from error
                if not re.fullmatch(r"[a-z_]+", surface):
                    raise RunnerError("restricted leakage source surface is non-canonical")
                path_length = struct.unpack(
                    "<I",
                    _read_exact_stream(
                        stream, 4, f"restricted source[{index}] path length"
                    ),
                )[0]
                if path_length == 0 or path_length > 4_096:
                    raise RunnerError("restricted leakage source path is outside its bound")
                try:
                    relative = _read_exact_stream(
                        stream, path_length, f"restricted source[{index}] path"
                    ).decode("utf-8")
                except UnicodeError as error:
                    raise RunnerError("restricted leakage source path is not UTF-8") from error
                if (
                    PurePosixPath(relative).is_absolute()
                    or PurePosixPath(relative).as_posix() != relative
                    or any(part in ("", ".", "..") for part in relative.split("/"))
                ):
                    raise RunnerError("restricted leakage source path is non-canonical")
                source_bytes = struct.unpack(
                    "<Q",
                    _read_exact_stream(
                        stream, 8, f"restricted source[{index}] byte length"
                    ),
                )[0]
                if source_bytes == 0 or source_bytes > leakage_audit.DEFAULT_MAX_FILE_BYTES:
                    raise RunnerError("restricted leakage source bytes are outside their bound")
                identity = (surface, relative)
                if ordinal != index or (previous is not None and identity <= previous):
                    raise RunnerError("restricted leakage sources are not uniquely ordered")
                previous = identity
                source_offset = stream.tell()
                digest = hashlib.sha256()
                remaining = source_bytes
                while remaining:
                    chunk = stream.read(min(remaining, 1024 * 1024))
                    if not chunk:
                        raise RunnerError(f"restricted source[{index}] is truncated")
                    digest.update(chunk)
                    remaining -= len(chunk)
                total_source_bytes += source_bytes
                if total_source_bytes > leakage_audit.DEFAULT_MAX_TOTAL_BYTES:
                    raise RunnerError("restricted leakage sources exceed their total bound")
                rows.append(
                    {
                        "surface": surface,
                        "relative_path": relative,
                        "source_offset": source_offset,
                        "source_bytes": source_bytes,
                        "source_sha256": digest.hexdigest(),
                    }
                )
            if stream.read(1):
                raise RunnerError("restricted leakage source archive has trailing bytes")
            final_metadata = os.fstat(stream.fileno())
    finally:
        os.close(descriptor)
    stable = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if any(
        getattr(metadata, field) != getattr(final_metadata, field) for field in stable
    ):
        raise RunnerError("restricted leakage source archive changed during replay")

    grouped_rows: dict[str, list[dict[str, Any]]] = {}
    for row in rows:
        grouped_rows.setdefault(row["surface"], []).append(row)
    replay_descriptor, replay_metadata = _open_regular_nofollow(path)
    if any(
        getattr(metadata, field) != getattr(replay_metadata, field) for field in stable
    ):
        os.close(replay_descriptor)
        raise RunnerError("restricted leakage source archive changed before binding replay")
    groups: dict[str, dict[str, Any]] = {}
    try:
        with os.fdopen(replay_descriptor, "rb", closefd=False) as stream:
            for surface, surface_rows in grouped_rows.items():
                binding = hashlib.sha256()
                binding.update(b"iroha:aps-leakage-source-binding:v1\0")
                binding.update(struct.pack("<Q", len(surface_rows)))
                source_total = 0
                for row in surface_rows:
                    stream.seek(row["source_offset"])
                    source_bytes = row["source_bytes"]
                    binding.update(struct.pack("<Q", source_bytes))
                    remaining = source_bytes
                    while remaining:
                        chunk = stream.read(min(remaining, 1024 * 1024))
                        if not chunk:
                            raise RunnerError("restricted leakage source changed during binding")
                        binding.update(chunk)
                        remaining -= len(chunk)
                    source_total += source_bytes
                groups[surface] = {
                    "source_sha256": binding.hexdigest(),
                    "source_bytes": source_total,
                    "source_count": len(surface_rows),
                    "rows": [
                        {
                            "relative_path": row["relative_path"],
                            "source_offset": row["source_offset"],
                            "source_sha256": row["source_sha256"],
                            "source_bytes": row["source_bytes"],
                        }
                        for row in surface_rows
                    ],
                }
            replay_final = os.fstat(stream.fileno())
    finally:
        os.close(replay_descriptor)
    if any(
        getattr(metadata, field) != getattr(replay_final, field) for field in stable
    ):
        raise RunnerError("restricted leakage source archive changed during binding replay")
    return groups


def _single_file_source_binding(path: Path) -> dict[str, Any]:
    """Recompute the Rust one-source binding over one stable archived file."""

    descriptor, metadata = _open_regular_nofollow(path)
    digest = hashlib.sha256()
    digest.update(b"iroha:aps-leakage-source-binding:v1\0")
    digest.update(struct.pack("<Q", 1))
    digest.update(struct.pack("<Q", metadata.st_size))
    consumed = 0
    try:
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            while chunk := stream.read(1024 * 1024):
                digest.update(chunk)
                consumed += len(chunk)
            final_metadata = os.fstat(stream.fileno())
    finally:
        os.close(descriptor)
    stable = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if consumed != metadata.st_size or any(
        getattr(metadata, field) != getattr(final_metadata, field) for field in stable
    ):
        raise RunnerError("restricted leakage source archive changed while bound")
    return {
        "source_sha256": digest.hexdigest(),
        "source_bytes": consumed,
        "source_count": 1,
    }


def _read_restricted_archive_row(path: Path, row: Mapping[str, Any]) -> bytes:
    descriptor, metadata = _open_regular_nofollow(path)
    offset = bounded_integer(
        row["source_offset"], 1, metadata.st_size, "restricted source offset"
    )
    length = bounded_integer(
        row["source_bytes"],
        1,
        leakage_audit.DEFAULT_MAX_FILE_BYTES,
        "restricted source bytes",
    )
    if offset + length > metadata.st_size:
        os.close(descriptor)
        raise RunnerError("restricted source row escapes its archive")
    try:
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            stream.seek(offset)
            raw = _read_exact_stream(stream, length, "restricted source row")
            final_metadata = os.fstat(stream.fileno())
    finally:
        os.close(descriptor)
    stable = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if any(
        getattr(metadata, field) != getattr(final_metadata, field) for field in stable
    ):
        raise RunnerError("restricted source archive changed while a row was read")
    if hashlib.sha256(raw).hexdigest() != row["source_sha256"]:
        raise RunnerError("restricted source row digest is false")
    return raw


def _validate_leakage_atomicity_observations(
    archive: Path,
    rows: Sequence[Mapping[str, Any]],
    participants: int,
    expected_peers: int,
) -> int:
    """Independently replay every retained full-topology atomicity observation."""

    if (
        expected_peers != (participants + 1) * VALIDATORS_PER_DATASPACE
        or len(rows) != expected_peers
    ):
        raise RunnerError("restricted atomicity evidence omits validators")
    count_fields = {
        "governance",
        "pools",
        "roots",
        "nullifiers",
        "commitments",
        "encrypted_outputs",
        "replay_markers",
        "receipts",
        "abort_markers",
        "staged_pool_heads",
        "staged_nullifiers",
        "staged_output_commitments",
        "replicated_staged_locks",
        "staged_locks",
    }
    expected_deltas = {
        "roots": participants,
        "nullifiers": participants * 2,
        "commitments": participants * 3,
        "encrypted_outputs": participants * 3,
        "replay_markers": 1,
        "receipts": 1,
    }
    total_checks = 0
    final_ledgers: set[str] = set()
    baseline_states: set[tuple[str, str, str, tuple[tuple[str, int], ...]]] = set()
    final_states: set[tuple[str, str, str, tuple[tuple[str, int], ...]]] = set()
    registered_states: list[
        tuple[
            tuple[str, str, str, tuple[tuple[str, int], ...]],
            int,
            int,
            int,
        ]
    ] = []
    for peer_index, row in enumerate(rows):
        try:
            document_text = _read_restricted_archive_row(archive, row).decode("utf-8")
        except UnicodeError as error:
            raise RunnerError("atomicity evidence is not UTF-8") from error
        document = strict_json_loads(
            document_text, f"atomicity evidence peer {peer_index}"
        )
        document = exact_fields(
            document,
            {"version", "peer_index", "registered", "observations"},
            f"atomicity evidence peer {peer_index}",
        )
        registered_state = _validate_fault_state_response(
            document["registered"],
            label=f"atomicity evidence peer {peer_index}.registered",
            expected_peer_index=peer_index,
        )
        registered_height = bounded_integer(
            document["registered"]["height"],
            1,
            MAX_OBSERVATION_COUNT,
            "registered atomicity observation height",
        )
        observations = document["observations"]
        if (
            document["version"] != 1
            or document["peer_index"] != peer_index
            or not isinstance(observations, list)
            or len(observations) < 3
        ):
            raise RunnerError("atomicity evidence has an invalid peer sequence")
        baseline_counts: dict[str, int] | None = None
        baseline_ledger: str | None = None
        finalized = 0
        peer_final_ledger: str | None = None
        empty_replicated_staged_commitment: str | None = None
        empty_staged_commitment: str | None = None
        previous_height: int | None = None
        first_height: int | None = None
        peer_final_state: tuple[str, str, str, tuple[tuple[str, int], ...]] | None = None
        for observation_index, item in enumerate(observations):
            observation = exact_fields(
                item,
                {
                    "peer_index",
                    "response_sha256",
                    "response_hex",
                    "height",
                    "commitment",
                    "ledger_commitment",
                    "replicated_staged_lock_commitment",
                    "staged_lock_commitment",
                    "counts",
                },
                f"atomicity peer {peer_index} observation {observation_index}",
            )
            if observation["peer_index"] != peer_index:
                raise RunnerError("atomicity observation changed validator identity")
            height = bounded_integer(
                observation["height"],
                0,
                MAX_OBSERVATION_COUNT,
                "atomicity observation height",
            )
            if previous_height is not None and height < previous_height:
                raise RunnerError("atomicity observation heights are not nondecreasing")
            if first_height is None:
                first_height = height
            previous_height = height
            for field in (
                "commitment",
                "ledger_commitment",
                "replicated_staged_lock_commitment",
                "staged_lock_commitment",
            ):
                canonical_iroha_hash_body(
                    observation[field], f"atomicity observation {field}"
                )
            response, response_document = _decode_bound_evidence_json_hex(
                observation["response_hex"],
                f"atomicity response peer {peer_index} observation {observation_index}",
            )
            if (
                hashlib.sha256(response).hexdigest() != observation["response_sha256"]
            ):
                raise RunnerError("atomicity observation response binding is false")
            response_document = exact_fields(
                response_document,
                {
                    "format_version",
                    "height",
                    "commitment",
                    "ledger_commitment",
                    "replicated_staged_lock_commitment",
                    "staged_lock_commitment",
                    "counts",
                },
                f"atomicity response peer {peer_index} observation {observation_index}",
            )
            if (
                response_document["format_version"] != 1
                or response_document["height"] != observation["height"]
                or response_document["commitment"] != observation["commitment"]
                or response_document["ledger_commitment"]
                != observation["ledger_commitment"]
                or response_document["replicated_staged_lock_commitment"]
                != observation["replicated_staged_lock_commitment"]
                or response_document["staged_lock_commitment"]
                != observation["staged_lock_commitment"]
                or response_document["counts"] != observation["counts"]
            ):
                raise RunnerError("atomicity observation projection differs from its raw response")
            counts = observation["counts"]
            if not isinstance(counts, dict) or set(counts) != count_fields:
                raise RunnerError("atomicity observation count vector is incomplete")
            normalized = {
                name: bounded_integer(
                    counts[name], 0, MAX_OBSERVATION_COUNT, f"atomicity.counts.{name}"
                )
                for name in count_fields
            }
            staged_pool_heads = normalized["staged_pool_heads"]
            staged_nullifiers = normalized["staged_nullifiers"]
            staged_outputs = normalized["staged_output_commitments"]
            staged_total = normalized["staged_locks"]
            replicated_staged_total = normalized["replicated_staged_locks"]
            if (
                staged_pool_heads > participants
                or staged_nullifiers != staged_pool_heads * 2
                or staged_outputs != staged_pool_heads * 3
                or staged_total
                != staged_pool_heads + staged_nullifiers + staged_outputs
            ):
                raise RunnerError("atomicity observation has an impossible staged-lock shape")
            if replicated_staged_total not in (0, 1 + participants * 9):
                raise RunnerError(
                    "atomicity observation has an impossible replicated staged-lock shape"
                )
            if replicated_staged_total == 0:
                if empty_replicated_staged_commitment is None:
                    empty_replicated_staged_commitment = observation[
                        "replicated_staged_lock_commitment"
                    ]
                elif (
                    observation["replicated_staged_lock_commitment"]
                    != empty_replicated_staged_commitment
                ):
                    raise RunnerError(
                        "empty replicated staged-lock commitment changed during observation"
                    )
            elif (
                empty_replicated_staged_commitment is not None
                and observation["replicated_staged_lock_commitment"]
                == empty_replicated_staged_commitment
            ):
                raise RunnerError(
                    "non-empty replicated staged locks reused the empty commitment"
                )
            if staged_total == 0:
                if empty_staged_commitment is None:
                    empty_staged_commitment = observation["staged_lock_commitment"]
                elif observation["staged_lock_commitment"] != empty_staged_commitment:
                    raise RunnerError("empty staged-lock commitment changed during observation")
            elif (
                empty_staged_commitment is not None
                and observation["staged_lock_commitment"] == empty_staged_commitment
            ):
                raise RunnerError("non-empty staged locks reused the empty commitment")
            ledger_normalized = {
                name: value
                for name, value in normalized.items()
                if name
                not in {
                    "staged_pool_heads",
                    "staged_nullifiers",
                    "staged_output_commitments",
                    "replicated_staged_locks",
                    "staged_locks",
                }
            }
            if baseline_counts is None:
                if (
                    staged_total != 0
                    or replicated_staged_total != 0
                    or empty_staged_commitment is None
                    or empty_replicated_staged_commitment is None
                ):
                    raise RunnerError("atomicity baseline contains staged locks")
                baseline_counts = ledger_normalized
                baseline_ledger = observation["ledger_commitment"]
                baseline_states.add(
                    (
                        baseline_ledger,
                        empty_replicated_staged_commitment,
                        empty_staged_commitment,
                        tuple(sorted(ledger_normalized.items())),
                    )
                )
                continue
            if (
                ledger_normalized == baseline_counts
                and observation["ledger_commitment"] == baseline_ledger
            ):
                if finalized:
                    raise RunnerError("atomicity evidence reverted after observing finalization")
                continue
            if observation["ledger_commitment"] == baseline_ledger:
                raise RunnerError("atomicity counts changed without a ledger transition")
            for name, delta in expected_deltas.items():
                if ledger_normalized[name] != baseline_counts[name] + delta:
                    raise RunnerError(
                        f"atomicity observation exposed a partial {name} transition"
                    )
            for name in ("governance", "pools", "abort_markers"):
                if ledger_normalized[name] != baseline_counts[name]:
                    raise RunnerError(
                        f"atomicity observation changed {name} outside finalization"
                    )
            if (
                staged_total != 0
                or replicated_staged_total != 0
                or observation["replicated_staged_lock_commitment"]
                != empty_replicated_staged_commitment
                or observation["staged_lock_commitment"] != empty_staged_commitment
            ):
                raise RunnerError("finalized atomicity observation retained staged locks")
            if peer_final_ledger is None:
                peer_final_ledger = observation["ledger_commitment"]
            elif peer_final_ledger != observation["ledger_commitment"]:
                raise RunnerError("atomicity evidence contains multiple finalized states")
            finalized += 1
            current_final_state = (
                observation["ledger_commitment"],
                observation["replicated_staged_lock_commitment"],
                observation["staged_lock_commitment"],
                tuple(sorted(normalized.items())),
            )
            if peer_final_state is None:
                peer_final_state = current_final_state
            elif peer_final_state != current_final_state:
                raise RunnerError("atomicity evidence contains divergent finalized state")
        if finalized == 0:
            raise RunnerError("atomicity evidence never observed finalization")
        if peer_final_ledger is None:
            raise RunnerError("atomicity evidence lacks a finalized ledger binding")
        final_ledgers.add(peer_final_ledger)
        if peer_final_state is None:
            raise RunnerError("atomicity evidence lacks a complete final state")
        final_states.add(peer_final_state)
        if first_height is None or previous_height is None:
            raise RunnerError("atomicity evidence lacks a complete height interval")
        registered_states.append(
            (registered_state, registered_height, first_height, previous_height)
        )
        total_checks += len(observations)
    if len(baseline_states) != 1:
        raise RunnerError("atomicity evidence validators disagree on baseline state")
    if len(final_ledgers) != 1 or len(final_states) != 1:
        raise RunnerError("atomicity evidence validators disagree on final state")
    baseline_ledger, empty_replicated, empty_local, baseline_counts_tuple = next(
        iter(baseline_states)
    )
    baseline_counts = dict(baseline_counts_tuple)
    expected_replicated = 1 + participants * 9
    replicated_commitment: str | None = None
    committee_commitments: dict[int, str] = {}
    for peer_index, (
        (ledger, registered_replicated, registered_local, counts_tuple),
        registered_height,
        first_height,
        last_height,
    ) in enumerate(registered_states):
        counts = dict(counts_tuple)
        if not first_height <= registered_height <= last_height:
            raise RunnerError(
                "registered Prepare-lock observation escapes the retained atomicity interval"
            )
        if (
            ledger != baseline_ledger
            or any(counts[field] != baseline_counts[field] for field in baseline_counts)
            or counts["replicated_staged_locks"] != expected_replicated
            or registered_replicated == empty_replicated
        ):
            raise RunnerError(
                "atomicity evidence lacks one complete registered replicated Prepare lock"
            )
        if replicated_commitment is None:
            replicated_commitment = registered_replicated
        elif registered_replicated != replicated_commitment:
            raise RunnerError(
                "atomicity evidence has divergent registered replicated Prepare locks"
            )
        if peer_index < VALIDATORS_PER_DATASPACE:
            if (
                counts["staged_pool_heads"] != 0
                or counts["staged_nullifiers"] != 0
                or counts["staged_output_commitments"] != 0
                or counts["staged_locks"] != 0
                or registered_local != empty_local
            ):
                raise RunnerError(
                    "atomicity evidence gives a global validator a committee-local lock"
                )
            continue
        if (
            counts["staged_pool_heads"] != 1
            or counts["staged_nullifiers"] != 2
            or counts["staged_output_commitments"] != 3
            or counts["staged_locks"] != 6
            or registered_local == empty_local
        ):
            raise RunnerError(
                "atomicity evidence lacks one complete registered committee-local leg lock"
            )
        committee = (
            peer_index - VALIDATORS_PER_DATASPACE
        ) // VALIDATORS_PER_DATASPACE
        existing = committee_commitments.setdefault(committee, registered_local)
        if existing != registered_local:
            raise RunnerError(
                "atomicity evidence has divergent registered committee-local locks"
            )
    if len(committee_commitments) != participants:
        raise RunnerError("atomicity evidence omits a registered participant committee")
    return total_checks


def _leakage_json_records(
    path: Path,
    *,
    label: str,
    fields: set[str],
    expected_peers: int | None = None,
) -> list[dict[str, Any]]:
    before = file_binding(path)
    document = read_json_file(path, label)
    if file_binding(path) != before:
        raise RunnerError(f"{label} changed during source replay")
    root = exact_fields(document, {"version", "records"}, label)
    if root["version"] != VERSION or not isinstance(root["records"], list) or not root["records"]:
        raise RunnerError(f"{label} has an invalid or empty record inventory")
    rows = [
        exact_fields(row, fields, f"{label}.records[{index}]")
        for index, row in enumerate(root["records"])
    ]
    if expected_peers is not None:
        indexes = [row["peer_index"] for row in rows]
        if indexes != list(range(expected_peers)):
            raise RunnerError(f"{label} does not cover every validator exactly once")
    for index, row in enumerate(rows):
        if isinstance(row["peer_index"], bool) or not isinstance(row["peer_index"], int):
            raise RunnerError(f"{label}.records[{index}].peer_index is invalid")
        for key, value in row.items():
            if key.endswith("sha256"):
                if not isinstance(value, str) or SHA256.fullmatch(value) is None or value == "0" * 64:
                    raise RunnerError(f"{label}.records[{index}].{key} is invalid")
            if key.endswith("bytes") and (
                isinstance(value, bool)
                or not isinstance(value, int)
                or value < (1 if key == "source_bytes" else 0)
            ):
                raise RunnerError(f"{label}.records[{index}].{key} is invalid")
    return rows


def derive_leakage_nonpacket_counts(evidence_dir: Path) -> dict[str, int]:
    """Independently replay the five non-packet count channels from final files."""

    peer_count = (PRIMARY_PARTICIPANTS + 1) * VALIDATORS_PER_DATASPACE
    block_path = regular_file_under(
        evidence_dir, PurePosixPath(SURFACE_FILES["block_wire_capture"]), "block capture"
    )
    events = _leakage_json_records(
        regular_file_under(
            evidence_dir, PurePosixPath(SURFACE_FILES["event_capture"]), "event capture"
        ),
        label="leakage events",
        fields={
            "peer_index",
            "source_sha256",
            "source_bytes",
        },
    )
    if len(events) != 1 or events[0]["peer_index"] != 0:
        raise RunnerError("leakage event capture is not one retained carrier event")
    queries = _leakage_json_records(
        regular_file_under(
            evidence_dir, PurePosixPath(SURFACE_FILES["query_capture"]), "query capture"
        ),
        label="leakage queries",
        fields={
            "peer_index",
            "source_sha256",
            "source_bytes",
        },
        expected_peers=peer_count,
    )
    logs = _leakage_json_records(
        regular_file_under(
            evidence_dir, PurePosixPath(SURFACE_FILES["operator_log"]), "operator capture"
        ),
        label="leakage operator logs",
        fields={
            "peer_index",
            "stdout_sha256",
            "stderr_sha256",
            "stdout_bytes",
            "stderr_bytes",
        },
        expected_peers=peer_count,
    )
    if any(row["stdout_bytes"] + row["stderr_bytes"] <= 0 for row in logs):
        raise RunnerError("leakage operator capture has an empty validator source")
    telemetry = _leakage_json_records(
        regular_file_under(
            evidence_dir,
            PurePosixPath(SURFACE_FILES["telemetry_capture"]),
            "telemetry capture",
        ),
        label="leakage telemetry",
        fields={
            "peer_index",
            "status_sha256",
            "status_bytes",
            "metrics_sha256",
            "metrics_bytes",
            "source_sha256",
            "source_bytes",
        },
        expected_peers=peer_count,
    )
    if any(
        row["status_bytes"] <= 0
        or row["metrics_bytes"] <= 0
        or row["source_bytes"]
        != row["status_bytes"] + row["metrics_bytes"] + 16
        for row in telemetry
    ):
        raise RunnerError("leakage telemetry omitted status or runtime metrics")
    return {
        "block_messages": _derive_leakage_block_count(block_path),
        "query_responses": len(queries),
        "event_records": len(events),
        "log_records": len(logs),
        "telemetry_records": len(telemetry),
    }


def _validate_retained_nonpacket_source_rows(
    evidence_dir: Path,
    archive: Path,
    groups: Mapping[str, Mapping[str, Any]],
) -> None:
    """Bind every public source-digest row to exact retained source bytes."""

    peer_count = (PRIMARY_PARTICIPANTS + 1) * VALIDATORS_PER_DATASPACE
    block_path = regular_file_under(
        evidence_dir,
        PurePosixPath(SURFACE_FILES["block_wire_capture"]),
        "leakage block wire",
    )
    block_rows = groups["block_wire_capture"]["rows"]
    block_binding = file_binding(block_path)
    if (
        len(block_rows) != 1
        or block_rows[0]["relative_path"] != "carrier-block-wire.bin"
        or block_rows[0]["source_sha256"] != block_binding["sha256"]
        or block_rows[0]["source_bytes"] != block_binding["bytes"]
    ):
        raise RunnerError("public block-wire artifact differs from its retained raw source")
    specifications = {
        "event_capture": (
            "leakage events",
            {
                "peer_index",
                "source_sha256",
                "source_bytes",
            },
            None,
        ),
        "query_capture": (
            "leakage queries",
            {
                "peer_index",
                "source_sha256",
                "source_bytes",
            },
            peer_count,
        ),
        "telemetry_capture": (
            "leakage telemetry",
            {
                "peer_index",
                "status_sha256",
                "status_bytes",
                "metrics_sha256",
                "metrics_bytes",
                "source_sha256",
                "source_bytes",
            },
            peer_count,
        ),
        "operator_log": (
            "leakage operator logs",
            {
                "peer_index",
                "stdout_sha256",
                "stderr_sha256",
                "stdout_bytes",
                "stderr_bytes",
            },
            peer_count,
        ),
    }
    for surface, (label, fields, expected_peers) in specifications.items():
        records = _leakage_json_records(
            regular_file_under(
                evidence_dir, PurePosixPath(SURFACE_FILES[surface]), label
            ),
            label=label,
            fields=fields,
            expected_peers=expected_peers,
        )
        source_rows = groups[surface]["rows"]
        if len(records) != len(source_rows):
            raise RunnerError(f"{surface} records omit retained raw sources")
        for index, (record, source_row) in enumerate(zip(records, source_rows)):
            source = _read_restricted_archive_row(archive, source_row)
            if surface in {"event_capture", "query_capture"}:
                if (
                    record["source_bytes"] != len(source)
                    or record["source_sha256"] != hashlib.sha256(source).hexdigest()
                ):
                    raise RunnerError(f"{surface}[{index}] has a false source binding")
            elif surface == "telemetry_capture":
                if len(source) < 16:
                    raise RunnerError("telemetry retained source is truncated")
                status_bytes = struct.unpack_from("<Q", source, 0)[0]
                metrics_offset = 8 + status_bytes
                if metrics_offset + 8 > len(source):
                    raise RunnerError("telemetry retained status length is invalid")
                metrics_bytes = struct.unpack_from("<Q", source, metrics_offset)[0]
                status = source[8:metrics_offset]
                metrics = source[metrics_offset + 8 :]
                if (
                    metrics_bytes != len(metrics)
                    or record["status_bytes"] != len(status)
                    or record["metrics_bytes"] != len(metrics)
                    or record["source_bytes"] != len(source)
                    or record["status_sha256"] != hashlib.sha256(status).hexdigest()
                    or record["metrics_sha256"] != hashlib.sha256(metrics).hexdigest()
                    or record["source_sha256"] != hashlib.sha256(source).hexdigest()
                ):
                    raise RunnerError(f"telemetry_capture[{index}] has a false source binding")
            else:
                if len(source) < 16:
                    raise RunnerError("operator retained source is truncated")
                stdout_bytes = struct.unpack_from("<Q", source, 0)[0]
                stderr_offset = 8 + stdout_bytes
                if stderr_offset + 8 > len(source):
                    raise RunnerError("operator retained stdout length is invalid")
                stderr_bytes = struct.unpack_from("<Q", source, stderr_offset)[0]
                stdout = source[8:stderr_offset]
                stderr = source[stderr_offset + 8 :]
                if (
                    stderr_bytes != len(stderr)
                    or record["stdout_bytes"] != len(stdout)
                    or record["stderr_bytes"] != len(stderr)
                    or record["stdout_sha256"] != hashlib.sha256(stdout).hexdigest()
                    or record["stderr_sha256"] != hashlib.sha256(stderr).hexdigest()
                ):
                    raise RunnerError(f"operator_log[{index}] has a false source binding")


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
        LEAKAGE_PAYLOAD_FIELDS,
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
    bounded_integer(
        payload["continuous_atomicity_checks"],
        (PRIMARY_PARTICIPANTS + 1) * VALIDATORS_PER_DATASPACE * 3,
        MAX_OBSERVATION_COUNT,
        "leakage.continuous_atomicity_checks",
    )
    bounded_integer(
        payload["partial_visible_observations"],
        0,
        0,
        "leakage.partial_visible_observations",
    )
    bounded_integer(
        payload["partial_spendable_observations"],
        0,
        0,
        "leakage.partial_spendable_observations",
    )
    counts = payload["traffic_counts"]
    if not isinstance(counts, dict) or set(counts) != set(
        leakage_audit.REQUIRED_COUNT_CHANNELS
    ):
        raise RunnerError("leakage traffic-count inventory is incomplete")
    normalized_counts = {
        channel: bounded_integer(
            counts[channel],
            1,
            MAX_OBSERVATION_COUNT,
            f"leakage.counts.{channel}",
        )
        for channel in leakage_audit.REQUIRED_COUNT_CHANNELS
    }
    provenance = exact_fields(
        payload["capture_provenance"],
        {"raw_pcap", "port_manifest", "ports", "packet_counts", "tcpdump"},
        "leakage.capture_provenance",
    )
    raw_pcap_binding = exact_fields(
        provenance["raw_pcap"], {"sha256", "bytes"}, "leakage.raw_pcap"
    )
    manifest_binding = exact_fields(
        provenance["port_manifest"],
        {"sha256", "bytes"},
        "leakage.port_manifest",
    )
    for label, binding in (
        ("raw_pcap", raw_pcap_binding),
        ("port_manifest", manifest_binding),
    ):
        if (
            not isinstance(binding["sha256"], str)
            or SHA256.fullmatch(binding["sha256"]) is None
            or binding["sha256"] == "0" * 64
        ):
            raise RunnerError(f"leakage {label} digest is invalid")
        bounded_integer(
            binding["bytes"], 1, leakage_audit.DEFAULT_MAX_FILE_BYTES, f"leakage.{label}.bytes"
        )
    try:
        port_document = exact_fields(
            provenance["ports"],
            capture_split.PORT_MANIFEST_FIELDS,
            "leakage.capture_provenance.ports",
        )
        groups = capture_split.validate_port_manifest(port_document)
    except (RunnerError, capture_split.CaptureSplitError) as error:
        raise RunnerError(f"leakage capture port binding is invalid: {error}") from error
    peer_count = (PRIMARY_PARTICIPANTS + 1) * VALIDATORS_PER_DATASPACE
    participant_visibilities = canonical_participant_visibilities(PRIMARY_PARTICIPANTS)
    expected_public_p2p = (
        1 + participant_visibilities.count(PUBLIC_PARTICIPANT_VISIBILITY)
    ) * VALIDATORS_PER_DATASPACE
    expected_restricted_p2p = participant_visibilities.count(
        RESTRICTED_PARTICIPANT_VISIBILITY
    ) * VALIDATORS_PER_DATASPACE
    if (
        len(groups["torii"]) != peer_count
        or len(groups["public_p2p"]) != expected_public_p2p
        or len(groups["restricted_p2p"]) != expected_restricted_p2p
    ):
        raise RunnerError("leakage capture ports do not cover the exact N=3 topology")
    if capture_split.canonical_port_manifest_binding(groups) != manifest_binding:
        raise RunnerError("leakage port-manifest binding is not derived from its port document")
    tcpdump = exact_fields(
        provenance["tcpdump"],
        {"stderr_base64", "stderr_sha256", "stderr_bytes", "statistics"},
        "leakage.capture_provenance.tcpdump",
    )
    try:
        tcpdump_stderr = base64.b64decode(tcpdump["stderr_base64"], validate=True)
    except (TypeError, ValueError) as error:
        raise RunnerError("leakage tcpdump stderr is not canonical base64") from error
    if (
        not tcpdump_stderr
        or base64.b64encode(tcpdump_stderr).decode("ascii")
        != tcpdump["stderr_base64"]
        or hashlib.sha256(tcpdump_stderr).hexdigest() != tcpdump["stderr_sha256"]
        or len(tcpdump_stderr) != tcpdump["stderr_bytes"]
    ):
        raise RunnerError("leakage tcpdump stderr binding is false")
    try:
        replayed_tcpdump_statistics = capture_split.parse_tcpdump_statistics(
            tcpdump_stderr
        )
    except capture_split.CaptureSplitError as error:
        raise RunnerError(f"leakage tcpdump statistics are invalid: {error}") from error
    if replayed_tcpdump_statistics != tcpdump["statistics"]:
        raise RunnerError("leakage tcpdump statistics differ from retained stderr")
    split_counts = exact_fields(
        provenance["packet_counts"],
        set(capture_split.PACKET_COUNT_FIELDS),
        "leakage.capture_provenance.packet_counts",
    )
    normalized_split_counts = {
        channel: bounded_integer(
            split_counts[channel],
            1,
            MAX_OBSERVATION_COUNT,
            f"leakage.packet_counts.{channel}",
        )
        for channel in split_counts
    }
    artifacts = payload["artifacts"]
    expected_surfaces = sorted(SURFACE_FILES)
    if not isinstance(artifacts, list) or len(artifacts) != len(expected_surfaces):
        raise RunnerError("leakage artifacts must cover every required surface")
    if evidence_dir.is_symlink() or not evidence_dir.is_dir():
        raise RunnerError("leakage evidence root must be a regular directory")
    by_surface: dict[str, tuple[Path, dict[str, Any]]] = {}
    source_claims: dict[str, dict[str, Any]] = {}
    derivative_rows: dict[str, list[dict[str, Any]]] = {}
    total_bytes = 0
    for index, item in enumerate(artifacts):
        row = exact_fields(
            item,
            LEAKAGE_ARTIFACT_FIELDS,
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
        if (
            not isinstance(row["source_sha256"], str)
            or SHA256.fullmatch(row["source_sha256"]) is None
            or row["source_sha256"] == "0" * 64
        ):
            raise RunnerError(f"leakage surface {surface} has an invalid source digest")
        bounded_integer(
            row["source_bytes"],
            1,
            leakage_audit.DEFAULT_MAX_TOTAL_BYTES,
            f"leakage.artifacts[{index}].source_bytes",
        )
        bounded_integer(
            row["source_count"],
            1,
            MAX_OBSERVATION_COUNT,
            f"leakage.artifacts[{index}].source_count",
        )
        if row["relative_name"].endswith(".pcapng") and (
            row["source_sha256"] != raw_pcap_binding["sha256"]
            or row["source_bytes"] != raw_pcap_binding["bytes"]
            or row["source_count"] != 1
        ):
            raise RunnerError(f"leakage packet surface {surface} is not bound to the raw pcap")
        if surface == "restricted_packet_source" and (
            binding != raw_pcap_binding
            or row["source_sha256"] != raw_pcap_binding["sha256"]
            or row["source_bytes"] != raw_pcap_binding["bytes"]
            or row["source_count"] != 1
        ):
            raise RunnerError("retained raw packet capture has a false provenance binding")
        derivative_kind = {
            "kura_artifact": "kura",
            "merge_artifact": "merge",
            "snapshot_artifact": "snapshot",
        }.get(surface)
        if derivative_kind is not None:
            source_count, source_bytes, rows = _validate_leakage_digest_derivative(
                source, derivative_kind
            )
            if (
                source_count != row["source_count"]
                or source_bytes != row["source_bytes"]
            ):
                raise RunnerError(
                    f"leakage surface {surface} source provenance differs from its digest frame"
                )
            derivative_rows[surface] = rows
        source_claims[surface] = {
            "source_sha256": row["source_sha256"],
            "source_bytes": row["source_bytes"],
            "source_count": row["source_count"],
        }
        by_surface[surface] = (source, binding)
    if set(by_surface) != set(SURFACE_FILES):
        raise RunnerError("leakage capture does not contain every required surface")
    restricted_source = by_surface["restricted_audit_source"][0]
    restricted_groups = _validate_restricted_leakage_source_archive(restricted_source)
    required_restricted_groups = {
        "block_wire_capture",
        "event_capture",
        "kura_artifact",
        "merge_artifact",
        "operator_log",
        "query_capture",
        "snapshot_artifact",
        "telemetry_capture",
        "coordinator_log",
        "confidential_da",
        "atomicity_observation",
    }
    if set(restricted_groups) != required_restricted_groups:
        raise RunnerError("restricted leakage source archive has an incomplete source inventory")
    expected_indexed_paths = {
        "query_capture": [f"peer-{index:03}.norito" for index in range(peer_count)],
        "telemetry_capture": [
            f"peer-{index:03}.status-metrics" for index in range(peer_count)
        ],
        "operator_log": [
            f"validator-{index:03}.stdout-stderr" for index in range(peer_count)
        ],
        "atomicity_observation": [
            f"peer-{index:03}.json" for index in range(peer_count)
        ],
    }
    for surface, expected_paths in expected_indexed_paths.items():
        if [row["relative_path"] for row in restricted_groups[surface]["rows"]] != expected_paths:
            raise RunnerError(f"restricted {surface} paths do not cover every validator")
    if [row["relative_path"] for row in restricted_groups["event_capture"]["rows"]] != [
        "event-000.norito"
    ]:
        raise RunnerError("restricted event source path is non-canonical")
    if [row["relative_path"] for row in restricted_groups["coordinator_log"]["rows"]] != [
        "coordinator-000/stdout-stderr.log"
    ]:
        raise RunnerError("restricted coordinator log path is non-canonical")
    replayed_atomicity_checks = _validate_leakage_atomicity_observations(
        restricted_source,
        restricted_groups["atomicity_observation"]["rows"],
        PRIMARY_PARTICIPANTS,
        peer_count,
    )
    if replayed_atomicity_checks != payload["continuous_atomicity_checks"]:
        raise RunnerError("atomicity check count is not backed by retained observations")
    _validate_retained_nonpacket_source_rows(
        evidence_dir, restricted_source, restricted_groups
    )
    for surface in required_restricted_groups.intersection(source_claims):
        claim = source_claims[surface]
        group = restricted_groups[surface]
        if claim != {
            "source_sha256": group["source_sha256"],
            "source_bytes": group["source_bytes"],
            "source_count": group["source_count"],
        }:
            raise RunnerError(
                f"leakage surface {surface} is not bound to its retained raw sources"
            )
    if source_claims["restricted_audit_source"] != _single_file_source_binding(
        restricted_source
    ):
        raise RunnerError("restricted leakage source artifact has a false self-binding")
    for surface, rows in derivative_rows.items():
        raw_rows = restricted_groups[surface]["rows"]
        expected_rows = [
            {
                "path_sha256": hashlib.sha256(
                    row["relative_path"].encode("utf-8")
                ).hexdigest(),
                "source_bytes": row["source_bytes"],
                "source_sha256": row["source_sha256"],
            }
            for row in raw_rows
        ]
        if rows != expected_rows:
            raise RunnerError(
                f"leakage derivative {surface} does not describe its retained raw sources"
            )
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
    try:
        replayed_split_counts = capture_split.derive_split_packet_counts(
            evidence_dir, groups
        )
        raw_capture = by_surface["restricted_packet_source"][0]
        with tempfile.TemporaryDirectory(prefix="aps-capture-replay-") as temporary:
            replay_root = Path(temporary)
            raw_replayed_counts = capture_split.packet_count_claims(
                capture_split.split_capture(
                    raw_capture,
                    replay_root,
                    groups,
                    expected_source_packets=replayed_tcpdump_statistics[
                        "captured_packets"
                    ],
                )
            )
            replayed_bindings = {
                name: file_binding(replay_root / relative)
                for name, relative in capture_split.OUTPUT_NAMES.items()
            }
        retained_bindings = {
            name: by_surface[
                {
                    "sanitized": "sanitized_capture",
                    "torii": "torii_capture",
                    "public_p2p": "public_p2p_capture",
                    "restricted_p2p": "restricted_p2p_capture",
                }[name]
            ][1]
            for name in capture_split.OUTPUT_NAMES
        }
    except (
        capture_split.CaptureSplitError,
        capture_split.pcapng.CaptureFormatError,
        OSError,
    ) as error:
        raise RunnerError(f"leakage split captures cannot be replayed: {error}") from error
    if raw_replayed_counts != replayed_split_counts:
        raise RunnerError("retained raw pcap produces different packet-count claims")
    if replayed_bindings != retained_bindings:
        raise RunnerError("retained split captures are not exact derivatives of the raw pcap")
    if replayed_split_counts != normalized_split_counts:
        raise RunnerError("leakage split-count claims differ from the final packet files")
    nonpacket_counts = derive_leakage_nonpacket_counts(evidence_dir)
    source_backed_counts = {
        "torii_request_packets": replayed_split_counts["torii_request_packets"],
        "torii_response_packets": replayed_split_counts["torii_response_packets"],
        "public_p2p_packets": replayed_split_counts["public_p2p_packets"],
        "restricted_p2p_packets": replayed_split_counts["restricted_p2p_packets"],
        **nonpacket_counts,
    }
    if normalized_counts != source_backed_counts:
        raise RunnerError("leakage traffic-count claims are not source-backed")
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
    participant_visibilities = canonical_participant_visibilities(
        job["participants"]
    )
    if (
        not isinstance(configuration, dict)
        or configuration.get("participant_visibilities") != participant_visibilities
    ):
        raise RunnerError(
            "request configuration does not bind the canonical participant "
            "visibility profile"
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
        "participant_visibilities": participant_visibilities,
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
            "traffic_count_channels": list(
                leakage_audit.REQUIRED_COUNT_CHANNELS
            ),
        }
    return request


def _process_group_exists(process_group: int) -> bool:
    """Return whether the exact harness-owned POSIX process group still exists."""

    try:
        os.killpg(process_group, 0)
    except ProcessLookupError:
        return False
    except PermissionError as error:
        raise RunnerError("cannot inspect the harness-owned process group") from error
    return True


def _terminate_owned_process_group(
    process: subprocess.Popen[bytes], process_group: int
) -> None:
    """Boundedly terminate only the new session created for one harness run."""

    if process_group != process.pid or process_group <= 1:
        raise RunnerError("refusing to terminate an unbound process group")
    if not _process_group_exists(process_group):
        process.wait(timeout=1)
        return
    try:
        os.killpg(process_group, signal.SIGTERM)
    except ProcessLookupError:
        process.wait(timeout=1)
        return
    deadline = time.monotonic() + 10.0
    while time.monotonic() < deadline and _process_group_exists(process_group):
        time.sleep(0.05)
    if _process_group_exists(process_group):
        try:
            os.killpg(process_group, signal.SIGKILL)
        except ProcessLookupError:
            pass
        deadline = time.monotonic() + 5.0
        while time.monotonic() < deadline and _process_group_exists(process_group):
            time.sleep(0.05)
    if _process_group_exists(process_group):
        raise RunnerError("harness-owned process group survived bounded termination")
    process.wait(timeout=1)


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
    JSON file, and (for fault or leakage jobs) write only their declared files
    beneath the evidence directory. Stdout/stderr never substitute for the response.
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
    evidence_dir.mkdir(mode=0o700)
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
    process: subprocess.Popen[bytes] | None = None
    try:
        with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
            process = subprocess.Popen(
                command,
                stdin=subprocess.DEVNULL,
                stdout=stdout,
                stderr=stderr,
                start_new_session=True,
            )
            process_group = process.pid
            try:
                returncode = process.wait(timeout=timeout_seconds)
            except subprocess.TimeoutExpired as error:
                _terminate_owned_process_group(process, process_group)
                raise RunnerError(
                    f"real-process harness exceeded its {timeout_seconds}-second deadline"
                ) from error
            if _process_group_exists(process_group):
                _terminate_owned_process_group(process, process_group)
                raise RunnerError(
                    "real-process harness exited while owned child processes remained"
                )
    except OSError as error:
        if process is not None and process.poll() is None:
            _terminate_owned_process_group(process, process.pid)
        temporary.cleanup()
        raise RunnerError(f"real-process harness invocation failed: {error}") from error
    except RunnerError:
        temporary.cleanup()
        raise
    if (
        expected_harness_binding is not None
        and verify_harness(harness) != dict(expected_harness_binding)
    ):
        temporary.cleanup()
        raise RunnerError("harness executable changed during invocation")
    if returncode != 0:
        try:
            with stderr_path.open("rb") as stream:
                stream.seek(max(0, stderr_path.stat().st_size - 2_000))
                stderr_tail = stream.read().decode("utf-8", errors="replace")
        except OSError:
            stderr_tail = "<stderr unavailable>"
        temporary.cleanup()
        raise RunnerError(
            f"real-process harness exited {returncode}: {stderr_tail}"
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
    if request.get("kind") == "benchmark" and any(evidence_dir.iterdir()):
        temporary.cleanup()
        raise RunnerError("benchmark harness wrote undeclared evidence files")
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
        root=publication_root,
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
    validate_campaign_timeout(plan["jobs"], timeout_seconds)
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
                        evidence_dir=evidence_dir,
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
                    provenance_path = (
                        publication
                        / "leakage"
                        / f"capture-provenance-{job['variant']}.json"
                    )
                    copy_bound_file(
                        response_path,
                        provenance_path,
                        expected=response_binding,
                    )
                    artifacts.append(
                        {
                            "kind": "leakage_capture_provenance",
                            **file_binding(provenance_path, relative_to=publication),
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
            path = publication / "leakage" / f"traffic-counts-{variant}.json"
            write_json(
                path,
                {"version": VERSION, "channels": leakage_counts[variant]},
            )
            count_paths[variant] = path
            artifacts.append(
                {
                    "kind": "traffic_count_manifest",
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
            or artifact["kind"] == "traffic_count_manifest"
        ]
        leakage_report_value = leakage_audit.run_audit(
            publication / "canary-manifest-v1.json",
            scannable,
            differential_left=publication / "leakage" / "left",
            differential_right=publication / "leakage" / "right",
            traffic_counts_left=count_paths["left"],
            traffic_counts_right=count_paths["right"],
        )
        if leakage_report_value["passed"] is not True:
            raise RunnerError(
                "leakage audit found a canary, public-shape, size, or "
                "traffic-count mismatch"
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


def validate_campaign_timeout(
    jobs: Sequence[Mapping[str, Any]], timeout_seconds: int
) -> None:
    """Reject a per-job deadline shorter than a job's protocol-time floor."""

    if timeout_seconds <= 0:
        raise RunnerError("harness timeout must be positive")
    if any(job.get("kind") == "fault" for job in jobs) and (
        timeout_seconds < FAULT_HARNESS_PROTOCOL_FLOOR_SECONDS
    ):
        raise RunnerError(
            "fault harness timeout is shorter than its mandatory activation-and-expiry "
            f"protocol floor ({FAULT_HARNESS_PROTOCOL_FLOOR_SECONDS} seconds)"
        )


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
    execute.add_argument(
        "--timeout-seconds", type=int, default=DEFAULT_HARNESS_TIMEOUT_SECONDS
    )
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
