#!/usr/bin/env python3
"""Validate private-settlement benchmark outcome evidence.

This dependency-free owner contains pure V1 protocol checks shared by release
tools. It never launches processes, reads environment variables, or grants
release qualification. Callers authenticate the request and retained file bytes
before passing decoded objects here. Run with --help for this module's scope.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from typing import Any, Mapping, Sequence

VERSION = 1
PROTOCOL = "AtomicPrivateSettlementV1"
TIMEOUT_SCOPE = "declared_job_and_stage_deadlines_v1"
BENCHMARK_PROTOCOL_DIRECTORY = "benchmark-protocol"
RUST_TERMINAL_FILE = "rust-result.json"
ADAPTER_OUTCOME_FILE = "adapter-outcome.json"
RUST_TERMINAL_FIELDS = frozenset({
    "version", "protocol", "request_id", "invocation_nonce", "request_sha256",
    "commit", "participants", "elapsed_ms", "outcome",
})
SUCCESS_RESULT_FIELDS = frozenset({
    "version", "protocol", "request_id", "invocation_nonce", "request_sha256",
    "commit", "participants", "mandatory_signed_rs16_da_rbc",
    "signed_rs16_da_observations", "authenticated_message_control",
    "process_inventory", "payload",
})
ADAPTER_OUTCOME_FIELDS = frozenset({
    "version", "protocol", "request_id", "invocation_nonce", "request_sha256",
    "commit", "participants", "elapsed_ms", "phase", "exit_code", "rust_terminal",
    "status", "reason",
})
DEADLINE_STAGES = frozenset({
    "coordinator_ack", "state_convergence", "transparent_consents",
    "transparent_balances", "native_amx_receipt", "canonical_carrier",
    "private_receipt",
})
WORKER_FAILURE_REASONS = frozenset({
    "execution_error", "worker_panic", "worker_spawn_error",
})
MAX_U64 = (1 << 64) - 1


class AccountingError(ValueError):
    """An outcome is malformed, contradictory, or bound to another attempt."""


def exact_fields(value: Any, fields: frozenset[str], label: str) -> dict[str, Any]:
    """Require an object with precisely the declared first-release fields."""

    if not isinstance(value, dict) or set(value) != fields:
        raise AccountingError(f"{label} has an incorrect field inventory")
    return value


def unsigned_milliseconds(value: Any, label: str, *, positive: bool = False) -> int:
    """Require actual bounded integer milliseconds, never booleans or estimates."""

    if type(value) is not int or not (1 if positive else 0) <= value <= MAX_U64:
        raise AccountingError(f"{label} is not bounded unsigned milliseconds")
    return value


def validate_benchmark_terminal(
    value: Any,
    *,
    request: Mapping[str, Any],
    request_sha256: str,
    exit_code: int,
) -> dict[str, Any]:
    """Bind a Rust terminal to its request and authentic process exit.

    Successful result semantics remain the responsibility of the canonical
    measurement validator. This boundary requires that full result; unsuccessful
    variants cannot smuggle metrics into the success-conditioned population.
    """

    terminal = exact_fields(value, RUST_TERMINAL_FIELDS, "Rust benchmark terminal")
    if (
        type(terminal["version"]) is not int or terminal["version"] != VERSION
        or terminal["protocol"] != PROTOCOL or request.get("kind") != "benchmark"
        or type(terminal["participants"]) is not int
        or terminal["participants"] not in (2, 3, 4, 8, 16)
        or type(exit_code) is not int
        or not isinstance(request_sha256, str)
        or re.fullmatch(r"[0-9a-f]{64}", request_sha256) is None
        or terminal["request_sha256"] != request_sha256
        or any(terminal[field] != request.get(field) for field in (
            "request_id", "invocation_nonce", "commit", "participants",
        ))
    ):
        raise AccountingError("Rust terminal does not bind the exact benchmark invocation")
    for field in ("request_id", "invocation_nonce"):
        if not isinstance(terminal[field], str) or re.fullmatch(
            r"[0-9a-f]{64}", terminal[field]
        ) is None:
            raise AccountingError("Rust terminal identity is malformed")
    if not isinstance(terminal["commit"], str) or re.fullmatch(
        r"(?:[0-9a-f]{40}|[0-9a-f]{64})", terminal["commit"]
    ) is None:
        raise AccountingError("Rust terminal source identity is malformed")
    elapsed = unsigned_milliseconds(terminal["elapsed_ms"], "terminal elapsed_ms")
    outcome = terminal["outcome"]
    if not isinstance(outcome, dict):
        raise AccountingError("Rust terminal outcome is not an object")
    kind = outcome.get("kind")
    if kind == "succeeded":
        exact_fields(outcome, frozenset({"kind", "result"}), "successful outcome")
        result = exact_fields(outcome["result"], SUCCESS_RESULT_FIELDS, "successful result")
        if (exit_code != 0 or type(result["version"]) is not int
                or type(result["participants"]) is not int
                or any(result[key] != terminal[key] for key in (
            "version", "protocol", "request_id", "invocation_nonce", "request_sha256",
            "commit", "participants",
        ))):
            raise AccountingError("successful Rust terminal contradicts its process exit or result")
    elif kind == "failed":
        exact_fields(outcome, frozenset({"kind", "stage", "reason"}), "failed outcome")
        if (
            exit_code <= 0 or outcome["stage"] != "benchmark_worker"
            or not isinstance(outcome["reason"], str)
            or outcome["reason"] not in WORKER_FAILURE_REASONS
        ):
            raise AccountingError("failed Rust terminal has an invalid reason or process exit")
    elif kind == "timed_out":
        exact_fields(outcome, frozenset({
            "kind", "stage", "budget_ms", "elapsed_ms",
        }), "timed-out outcome")
        if (
            exit_code <= 0 or not isinstance(outcome["stage"], str)
            or outcome["stage"] not in DEADLINE_STAGES
        ):
            raise AccountingError("Rust deadline has an undeclared stage or contradictory exit")
        budget = unsigned_milliseconds(outcome["budget_ms"], "deadline budget_ms", positive=True)
        duration = unsigned_milliseconds(outcome["elapsed_ms"], "deadline elapsed_ms")
        if duration < budget or duration > elapsed:
            raise AccountingError("Rust deadline was not exhausted within the observed invocation")
    else:
        raise AccountingError("Rust terminal has an unknown outcome kind")
    return terminal


def validate_adapter_outcome(
    value: Any, *, request: Mapping[str, Any], request_sha256: str,
) -> dict[str, Any]:
    """Validate the adapter's bounded transport observation without inferring success."""

    result = exact_fields(value, ADAPTER_OUTCOME_FIELDS, "benchmark adapter outcome")
    if (
        type(result["version"]) is not int or result["version"] != VERSION
        or result["protocol"] != PROTOCOL
        or result["request_sha256"] != request_sha256
        or type(result["participants"]) is not int
        or any(result[key] != request.get(key) for key in (
            "request_id", "invocation_nonce", "commit", "participants",
        ))
        or (result["exit_code"] is not None and type(result["exit_code"]) is not int)
    ):
        raise AccountingError("adapter outcome does not bind the benchmark invocation")
    unsigned_milliseconds(result["elapsed_ms"], "adapter elapsed_ms")
    binding = result["rust_terminal"]
    if binding is not None:
        exact_fields(binding, frozenset({"sha256", "bytes"}), "retained Rust terminal binding")
        if (
            not isinstance(binding["sha256"], str)
            or re.fullmatch(r"[0-9a-f]{64}", binding["sha256"]) is None
            or type(binding["bytes"]) is not int or not 0 < binding["bytes"] <= 16 * 1024 * 1024
        ):
            raise AccountingError("retained Rust terminal binding is malformed")
    shape = (result["status"], result["reason"], result["phase"])
    if not all(isinstance(item, str) for item in shape):
        raise AccountingError("adapter outcome codes are malformed")
    exit_code = result["exit_code"]
    if shape == ("succeeded", "rust_succeeded", "measurement_validation"):
        valid = exit_code == 0 and binding is not None
    elif shape in {
        ("failed", "rust_failed", "terminal_validation"),
        ("timed_out", "rust_timed_out", "terminal_validation"),
    }:
        valid = exit_code is not None and exit_code > 0 and binding is not None
    elif shape == ("failed", "validator_build_failed", "validator_build"):
        valid = exit_code is not None and exit_code != 0 and binding is None
    elif shape in {
        ("failed", "validator_build_spawn_failed", "validator_build"),
        ("failed", "benchmark_process_spawn_failed", "benchmark_process"),
    }:
        valid = exit_code is None and binding is None
    elif shape == ("incomplete", "rust_terminal_missing", "terminal_validation"):
        valid = exit_code is not None and binding is None
    elif result["status"] == "incomplete" and result["reason"] == "adapter_interrupted":
        valid = result["phase"] in {
            "validator_build", "validator_identity", "benchmark_process",
            "terminal_validation", "measurement_validation",
        }
    elif shape in {
        ("invalid", "rust_terminal_invalid", "terminal_validation"),
        ("invalid", "request_changed", "terminal_validation"),
        ("invalid", "measurement_invalid", "measurement_validation"),
        ("invalid", "validator_identity_invalid", "validator_identity"),
    }:
        valid = exit_code is not None
    else:
        valid = False
    if not valid:
        raise AccountingError("adapter outcome contradicts its phase, reason, exit or terminal")
    return result


# Registered-scope reduction consumes authenticated retained bytes. These names
# describe the single first-release producer contract, not a legacy adapter.
SCOPE_FIELDS = frozenset({
    "version", "protocol", "scope_id", "previous_scope_sha256", "registered_ns",
    "stopping_policy", "deadline_policy", "campaigns",
})
POLICY_FIELDS = frozenset({"timeout_scope", "outer_timeout_ms", "rust_deadline_budgets_ms"})
PLAN_FIELDS = frozenset({
    "version", "protocol", "commit", "worktree_clean", "publication_evidence",
    "execution_required", "harness", "harness_contract", "benchmark_baseline", "hardware",
    "canary_manifest", "configuration_manifest", "requirements", "jobs", "benchmark_accounting",
})
BENCHMARK_JOB_FIELDS = frozenset({
    "request_id", "kind", "profile", "participants", "seed", "run", "warmup",
    "configuration_sha256",
})
PACKET_FIELDS = frozenset({
    "request_id", "request", "started", "process", "adapter", "rust_terminal",
    "response", "response_outcome", "validation", "sample",
})
ACCOUNTING_IDENTITY_FIELDS = frozenset({
    "scope_sha256", "campaign_id", "plan_sha256", "attempt_id",
})
START_FIELDS = frozenset({
    "version", "protocol", "request_id", "invocation_nonce", "command", "request", "harness",
    "timeout_seconds", "started_ns",
}) | ACCOUNTING_IDENTITY_FIELDS
PROCESS_FIELDS = frozenset({
    "version", "protocol", "request_id", "invocation_nonce", "finished_ns", "pid", "exit_code",
    "timed_out", "error", "passed", "retained_files", "completion_kind", "elapsed_ms",
    "owned_process_group_gone", "bindings_unchanged",
}) | ACCOUNTING_IDENTITY_FIELDS
CLOSURE_FIELDS = frozenset({
    "version", "protocol", "scope_sha256", "campaign_id", "plan_sha256", "closed_ns",
    "quiescent", "started_request_ids", "reason",
})
VALIDATION_BASE_FIELDS = frozenset({
    "version", "protocol", "ordinal", "request_id", "kind", "invocation_nonce", "passed",
    "finished_ns", "validation_kind",
}) | ACCOUNTING_IDENTITY_FIELDS
COUNT_FIELDS = ("planned", "attempted", "not_started", "succeeded", "failed", "timed_out", "incomplete")
MAX_DOCUMENT_BYTES = 16 * 1024 * 1024


def accounting_canonical_bytes(value: Any) -> bytes:
    """Encode identity coordinates exactly like the runner's object_digest."""

    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True,
                          separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError, UnicodeError) as error:
        raise AccountingError("accounting value is not finite JSON") from error


def accounting_file_binding(raw: bytes) -> dict[str, Any]:
    """Bind the original retained bytes, including their actual JSON formatting."""

    if type(raw) is not bytes or not 0 < len(raw) <= MAX_DOCUMENT_BYTES:
        raise AccountingError("accounting document is missing or exceeds its byte bound")
    return {"sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}


def _document(raw: bytes, label: str) -> dict[str, Any]:
    accounting_file_binding(raw)

    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result = {}
        for key, value in items:
            if key in result:
                raise AccountingError(f"{label} repeats a JSON field")
            result[key] = value
        return result

    def invalid_constant(_: str) -> None:
        raise AccountingError(f"{label} contains a non-finite number")

    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=pairs, parse_constant=invalid_constant)
        # JSON's valid exponent syntax can overflow Python's float without using NaN/Infinity literals.
        accounting_canonical_bytes(value)
    except (ValueError, UnicodeError, RecursionError) as error:
        raise AccountingError(f"{label} is not strict bounded JSON") from error
    if not isinstance(value, dict):
        raise AccountingError(f"{label} must be an object")
    return value


def _digest(value: Any, label: str) -> str:
    if (not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None
            or value == "0" * 64):
        raise AccountingError(f"{label} must be a nonzero canonical SHA-256 identity")
    return value


def _campaign_id(value: Any) -> str:
    if not isinstance(value, str) or re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", value) is None:
        raise AccountingError("campaign_id must be a canonical registered slug")
    return value


def _binding(value: Any, label: str) -> dict[str, Any]:
    result = exact_fields(value, frozenset({"sha256", "bytes"}), label)
    _digest(result["sha256"], label)
    unsigned_milliseconds(result["bytes"], label, positive=True)
    return result


def _header(value: Mapping[str, Any], label: str) -> None:
    if type(value.get("version")) is not int or value["version"] != VERSION or value.get("protocol") != PROTOCOL:
        raise AccountingError(f"{label} has an invalid protocol header")


def _same(left: Any, right: Any, label: str) -> None:
    if accounting_canonical_bytes(left) != accounting_canonical_bytes(right):
        raise AccountingError(f"{label} differs from its bound value")


def validate_deadline_policy(value: Any) -> dict[str, Any]:
    """Require one frozen outer invocation budget and all declared Rust budgets."""

    policy = exact_fields(value, POLICY_FIELDS, "deadline policy")
    if policy["timeout_scope"] != TIMEOUT_SCOPE:
        raise AccountingError("deadline policy has an unknown timeout scope")
    outer = unsigned_milliseconds(policy["outer_timeout_ms"], "outer timeout", positive=True)
    if outer % 1_000:
        raise AccountingError("outer timeout must be an exact number of runner seconds")
    stages = exact_fields(policy["rust_deadline_budgets_ms"], DEADLINE_STAGES, "Rust deadlines")
    for stage, budget in stages.items():
        unsigned_milliseconds(budget, stage, positive=True)
        if budget != 300_000:
            raise AccountingError("Rust deadline differs from the current declared completion budget")
    return policy


# TODO: Wire this pure plan only together with persistent benchmark process
# ownership and its mandatory evidence contract; it is not an execution selector.
BENCHMARK_SESSION_PARTICIPANTS = (2, 3, 4, 8, 16)
BENCHMARK_SESSION_PROFILES = ("private", "transparent_control")
# Preserve the existing planner's maximum benchmark inventory before allocation.
MAX_PLANNED_BENCHMARK_ATTEMPTS = 20_000


def build_benchmark_session_plan(
    configuration_sha256: Mapping[int, str], seeds: Sequence[int],
    workload_sha256: Mapping[int, str], *, warmups_per_session: int,
    measured_per_profile: int,
) -> dict[str, list[dict[str, Any]]]:
    """Derive immutable network sessions and their ordered settlement jobs.

    Workload commitments are profile-independent: each topology/seed uses the
    same economic schedule in both profiles. Measured allocation is balanced in
    ascending seed order, with every independent session contributing at least
    one measurement. Warmups are required separately on every such session.
    Matched profile sessions are adjacent; the first profile alternates by seed
    index. This function does not construct a network or attest executed warmups.
    """
    for label, value in (("configuration", configuration_sha256), ("workload", workload_sha256)):
        if (not isinstance(value, Mapping)
                or any(type(key) is not int for key in value)
                or set(value) != set(BENCHMARK_SESSION_PARTICIPANTS)):
            raise AccountingError(f"session {label} must bind every exact participant count")
        for digest in value.values():
            _digest(digest, f"session {label}")
    if (not isinstance(seeds, (list, tuple)) or not 10 <= len(seeds) <= 256
            or any(type(seed) is not int or not 0 <= seed < 1 << 64 for seed in seeds)
            or list(seeds) != sorted(set(seeds))):
        raise AccountingError("session seeds must be 10..256 unique sorted unsigned integers")
    if type(warmups_per_session) is not int or not 5 <= warmups_per_session <= 1_000:
        raise AccountingError("warmups per session must be an integer in 5..1000")
    if (type(measured_per_profile) is not int or not 30 <= measured_per_profile <= 1_000
            or measured_per_profile < len(seeds)):
        raise AccountingError("measured allocation must give every session at least one measurement")
    total = len(BENCHMARK_SESSION_PROFILES) * len(BENCHMARK_SESSION_PARTICIPANTS) * (
        len(seeds) * warmups_per_session + measured_per_profile)
    if total > MAX_PLANNED_BENCHMARK_ATTEMPTS:
        raise AccountingError("session allocation exceeds the bounded benchmark job inventory")
    quotient, remainder = divmod(measured_per_profile, len(seeds))
    sessions, jobs = [], []
    for participants in BENCHMARK_SESSION_PARTICIPANTS:
        for seed_index, seed in enumerate(seeds):
            measured = quotient + int(seed_index < remainder)
            # Keep paired profiles adjacent and counterbalance their order.
            # This scheduled order does not prove the absence of confounding.
            profiles = BENCHMARK_SESSION_PROFILES if seed_index % 2 == 0 else BENCHMARK_SESSION_PROFILES[::-1]
            for profile in profiles:
                coordinates = {
                    "profile": profile, "participants": participants, "seed": seed,
                    "configuration_sha256": configuration_sha256[participants],
                    "workload_sha256": workload_sha256[participants],
                    "warmup_attempts": warmups_per_session, "measured_attempts": measured,
                }
                session_id = hashlib.sha256(accounting_canonical_bytes({
                    "domain": "iroha:private-settlement:benchmark-session-plan:v1", **coordinates,
                })).hexdigest()
                sessions.append({"session_id": session_id, **coordinates})
                for index in range(warmups_per_session + measured):
                    body = {
                        "kind": "benchmark", "profile": profile, "participants": participants,
                        "seed": seed, "session_id": session_id, "session_attempt_index": index,
                        "warmup": index < warmups_per_session,
                        "configuration_sha256": configuration_sha256[participants],
                        "workload_sha256": workload_sha256[participants],
                    }
                    jobs.append({"request_id": hashlib.sha256(accounting_canonical_bytes(body)).hexdigest(), **body})
    return {"sessions": sessions, "jobs": jobs}


def validate_benchmark_session_plan(
    value: Any, configuration_sha256: Mapping[int, str], seeds: Sequence[int],
    workload_sha256: Mapping[int, str], *, warmups_per_session: int,
    measured_per_profile: int,
) -> dict[str, list[dict[str, Any]]]:
    """Require the exact regenerated session and attempt inventory, not counters."""
    expected = build_benchmark_session_plan(
        configuration_sha256, seeds, workload_sha256,
        warmups_per_session=warmups_per_session, measured_per_profile=measured_per_profile)
    _same(value, expected, "canonical benchmark session plan")
    return expected


def validate_benchmark_session_dispatch_prefix(
    configuration_sha256: Mapping[int, str], seeds: Sequence[int],
    workload_sha256: Mapping[int, str], *, warmups_per_session: int,
    measured_per_profile: int, full_jobs: Sequence[Mapping[str, Any]],
    started_request_ids: Sequence[str], job_states: Sequence[Mapping[str, str]],
) -> None:
    """Check session scheduling against the authenticated campaign failure cut.

    Call only after the real reducer has authenticated its quiescent closure
    and classified each retained attempt of every job kind. job_states is an
    internal projection of those rows, never a serialized producer assertion
    or a source of evidence classification. This function cannot replace the
    collector, the real typed reducer, or authoritative quiescent closure.
    """
    expected = build_benchmark_session_plan(
        configuration_sha256, seeds, workload_sha256,
        warmups_per_session=warmups_per_session, measured_per_profile=measured_per_profile)
    if not isinstance(full_jobs, (list, tuple)) or not full_jobs:
        raise AccountingError("session dispatch requires the complete campaign job inventory")
    ids, benchmark_jobs, ordinals = [], [], []
    for ordinal, job in enumerate(full_jobs):
        if not isinstance(job, Mapping) or not isinstance(job.get("kind"), str) or job["kind"] not in {"fault", "benchmark", "leakage"}:
            raise AccountingError("session campaign contains a malformed job")
        request_id = _digest(job.get("request_id"), "session job request identity")
        body = {key: value for key, value in job.items() if key != "request_id"}
        if hashlib.sha256(accounting_canonical_bytes(body)).hexdigest() != request_id:
            raise AccountingError("session job identity differs from its planned coordinates")
        ids.append(request_id)
        if job["kind"] == "benchmark":
            benchmark_jobs.append(dict(job))
            ordinals.append(ordinal)
    _same(benchmark_jobs, expected["jobs"], "complete benchmark session job inventory")
    if ordinals != list(range(ordinals[0], ordinals[-1] + 1)):
        raise AccountingError("benchmark sessions are interrupted by a nonbenchmark job")
    if len(ids) != len(set(ids)):
        raise AccountingError("session campaign contains a duplicate planned request")
    if (not isinstance(started_request_ids, (list, tuple))
            or list(started_request_ids) != ids[:len(started_request_ids)]):
        raise AccountingError("session campaign durable starts are not a complete-plan prefix")
    if (not isinstance(job_states, (list, tuple))
            or len(job_states) != len(full_jobs)):
        raise AccountingError("session outcome inventory omits planned attempts")
    for ordinal, (job, row) in enumerate(zip(full_jobs, job_states)):
        exact_fields(row, frozenset({"request_id", "state"}), "reduced session state")
        if (row["request_id"] != job["request_id"]
                or not isinstance(row["state"], str) or row["state"] not in {"succeeded", "failed", "timed_out", "incomplete", "not_started"}):
            raise AccountingError("session outcome is unplanned, reordered or unclassified")
        started = ordinal < len(started_request_ids)
        if started == (row["state"] == "not_started"):
            raise AccountingError("session outcome disagrees with the durable start inventory")
        if started and row["state"] != "succeeded" and len(started_request_ids) > ordinal + 1:
            raise AccountingError("campaign started a full-plan job after its unsuccessful attempt")


def registered_attempt_id(scope_sha256: str, campaign_id: str, plan_sha256: str,
                          request_id: str) -> str:
    """Derive a unique planned benchmark invocation across registered campaigns."""

    coordinates = {"scope_sha256": scope_sha256, "campaign_id": campaign_id,
                   "plan_sha256": plan_sha256, "request_id": request_id}
    for key, value in coordinates.items():
        if key == "campaign_id":
            _campaign_id(value)
        else:
            _digest(value, key)
    return hashlib.sha256(accounting_canonical_bytes({
        "domain": "iroha:private-settlement:registered-attempt:v1", **coordinates,
    })).hexdigest()


def _record_identity(value: Mapping[str, Any], identity: Mapping[str, Any], label: str) -> None:
    _header(value, label)
    for key, expected in identity.items():
        _same(value.get(key), expected, f"{label}.{key}")


def _counter(rows: Sequence[Mapping[str, Any]]) -> dict[str, int]:
    counts = {key: 0 for key in COUNT_FIELDS}
    counts["planned"] = len(rows)
    for row in rows:
        counts[row["state"]] += 1
        if row["state"] != "not_started":
            counts["attempted"] += 1
    if (counts["planned"] != counts["not_started"] + counts["attempted"] or
            counts["attempted"] != sum(counts[key] for key in ("succeeded", "failed", "timed_out", "incomplete"))):
        raise AccountingError("accounting partition is inconsistent")
    return counts


def _request_join(request: Mapping[str, Any], start: Mapping[str, Any], plan: Mapping[str, Any],
                  job: Mapping[str, Any], raw: bytes) -> None:
    _header(request, "request")
    if request.get("kind") != "benchmark":
        raise AccountingError("benchmark accounting received another request kind")
    for key in ("request_id", "participants", "seed", "run", "configuration_sha256"):
        _same(request.get(key), job[key], f"request.{key}")
    for key, expected in {
        "commit": plan["commit"], "hardware_sha256": plan["hardware"]["sha256"],
        "hardware_profile_sha256": plan["hardware"]["profile_sha256"],
        "invocation_nonce": start["invocation_nonce"],
    }.items():
        _same(request.get(key), expected, f"request.{key}")
    payload = request.get("payload")
    if not isinstance(payload, dict):
        raise AccountingError("benchmark request has no payload")
    for key in ("profile", "warmup"):
        _same(payload.get(key), job[key], f"request.payload.{key}")
    _same(_binding(start["request"], "start.request"), accounting_file_binding(raw), "request bytes")
    _same(_binding(start["harness"], "start.harness"), plan["harness"], "harness executable")


def _process_record(value: Any, identity: Mapping[str, Any], start: Mapping[str, Any],
                    closed_ns: int, policy: Mapping[str, Any]) -> dict[str, Any]:
    process = exact_fields(value, PROCESS_FIELDS, "process outcome")
    _record_identity(process, identity, "process outcome")
    finished = unsigned_milliseconds(process["finished_ns"], "process finished_ns", positive=True)
    if not start["started_ns"] <= finished <= closed_ns:
        raise AccountingError("process outcome is outside the registered campaign cut")
    elapsed = unsigned_milliseconds(process["elapsed_ms"], "process elapsed_ms")
    for key in ("timed_out", "passed", "owned_process_group_gone", "bindings_unchanged"):
        if type(process[key]) is not bool:
            raise AccountingError("process outcome flags must be actual booleans")
    if not process["bindings_unchanged"]:
        raise AccountingError("process outcome reports changed bound inputs")
    if not isinstance(process["retained_files"], list) or (process["error"] is not None and not isinstance(process["error"], str)):
        raise AccountingError("process diagnostic fields are malformed")
    code, pid, kind = process["exit_code"], process["pid"], process["completion_kind"]
    if code is not None and type(code) is not int:
        raise AccountingError("process exit code must be an actual integer")
    if pid is not None and (type(pid) is not int or pid <= 0):
        raise AccountingError("process PID is malformed")
    if kind == "spawn_failed":
        valid = code is None and pid is None and not process["passed"] and not process["timed_out"]
    elif kind == "outer_deadline":
        valid = (pid is not None and process["timed_out"] and not process["passed"]
                 and elapsed >= policy["outer_timeout_ms"])
    elif kind == "exited":
        valid = pid is not None and code is not None and not process["timed_out"]
        if process["passed"] and code != 0:
            valid = False
    elif kind == "interrupted":
        valid = not process["passed"] and not process["timed_out"]
    else:
        valid = False
    if not valid:
        raise AccountingError("process completion contradicts its typed cause or observed exit")
    return process


def _validation_record(value: Any, identity: Mapping[str, Any], ordinal: int, start: Mapping[str, Any],
                       closed_ns: int) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise AccountingError("validation outcome must be an object")
    kind = value.get("validation_kind")
    accepted = kind == "accepted"
    fields = VALIDATION_BASE_FIELDS | (frozenset({"response", "sample"}) if accepted else frozenset({"stage", "error"}))
    validation = exact_fields(value, fields, "validation outcome")
    _record_identity(validation, identity, "validation outcome")
    if (type(validation["ordinal"]) is not int or validation["ordinal"] != ordinal
            or validation["kind"] != "benchmark" or type(validation["passed"]) is not bool
            or validation["passed"] != accepted):
        raise AccountingError("validation outcome has contradictory job coordinates")
    finished = unsigned_milliseconds(validation["finished_ns"], "validation finished_ns", positive=True)
    if not start["started_ns"] <= finished <= closed_ns:
        raise AccountingError("validation outcome is outside the registered campaign cut")
    if kind not in {"accepted", "rejected", "interrupted", "publication_failed", "not_validated"}:
        raise AccountingError("validation outcome has an undeclared typed cause")
    if not accepted and (not isinstance(validation["stage"], str) or not isinstance(validation["error"], str)):
        raise AccountingError("validation diagnostics are malformed")
    if kind == "rejected":
        raise AccountingError("semantic validation rejected the retained evidence")
    return validation


def _response_record(raw: bytes | None, response_raw: bytes | None) -> dict[str, Any] | None:
    if raw is None:
        return None
    record = _document(raw, "response outcome")
    _header(record, "response outcome")
    if type(record.get("passed")) is not bool:
        raise AccountingError("response outcome passed is not a boolean")
    if record["passed"]:
        exact_fields(record, frozenset({"version", "protocol", "passed", "response"}), "response outcome")
        if response_raw is None:
            raise AccountingError("accepted response outcome has no retained response")
        _same(_binding(record["response"], "response binding"), accounting_file_binding(response_raw), "response bytes")
    else:
        exact_fields(record, frozenset({"version", "protocol", "passed", "error", "retained_files"}), "response outcome")
        if not isinstance(record["error"], str) or not isinstance(record["retained_files"], list):
            raise AccountingError("response diagnostic fields are malformed")
    return record


def _terminal_without_exit(value: Any, request: Mapping[str, Any], request_sha: str) -> None:
    """Reject malformed orphan terminals without inventing a process exit observation."""

    terminal = exact_fields(value, RUST_TERMINAL_FIELDS, "orphan Rust terminal")
    _header(terminal, "orphan Rust terminal")
    for key in ("request_id", "invocation_nonce", "commit", "participants"):
        _same(terminal[key], request[key], f"orphan terminal.{key}")
    _same(terminal["request_sha256"], request_sha, "orphan terminal.request_sha256")
    elapsed = unsigned_milliseconds(terminal["elapsed_ms"], "orphan terminal elapsed")
    outcome = terminal["outcome"]
    if not isinstance(outcome, dict):
        raise AccountingError("orphan terminal outcome is malformed")
    if outcome.get("kind") == "succeeded":
        exact_fields(outcome, frozenset({"kind", "result"}), "orphan success")
        result = exact_fields(outcome["result"], SUCCESS_RESULT_FIELDS, "orphan successful result")
        for key in ("version", "protocol", "request_id", "invocation_nonce", "request_sha256", "commit", "participants"):
            _same(result[key], terminal[key], f"orphan successful result.{key}")
    elif outcome.get("kind") == "failed":
        exact_fields(outcome, frozenset({"kind", "stage", "reason"}), "orphan failure")
        if outcome["stage"] != "benchmark_worker" or outcome["reason"] not in WORKER_FAILURE_REASONS:
            raise AccountingError("orphan worker failure has an unknown bounded cause")
    elif outcome.get("kind") == "timed_out":
        exact_fields(outcome, frozenset({"kind", "stage", "budget_ms", "elapsed_ms"}), "orphan deadline")
        budget = unsigned_milliseconds(outcome["budget_ms"], "orphan deadline budget", positive=True)
        duration = unsigned_milliseconds(outcome["elapsed_ms"], "orphan deadline elapsed")
        if outcome["stage"] not in DEADLINE_STAGES or not budget <= duration <= elapsed:
            raise AccountingError("orphan deadline has an invalid stage or duration")
    else:
        raise AccountingError("orphan terminal has an unknown outcome kind")


def _successful_join(packet: Mapping[str, Any], request: Mapping[str, Any], result: Mapping[str, Any],
                     validation: Mapping[str, Any], identity: Mapping[str, Any], plan: Mapping[str, Any],
                     job: Mapping[str, Any]) -> dict[str, Any]:
    response = _document(packet["response"], "response")
    _header(response, "response")
    for key in ("request_id", "invocation_nonce", "commit", "participants", "hardware_sha256",
                "hardware_profile_sha256", "configuration_sha256"):
        _same(response.get(key), request[key], f"response.{key}")
    if response.get("kind") != "benchmark" or response.get("passed") is not True:
        raise AccountingError("successful response has a contradictory header")
    for key in ("payload", "process_inventory", "mandatory_signed_rs16_da_rbc",
                "signed_rs16_da_observations", "authenticated_message_control"):
        _same(response.get(key), result[key], f"successful response.{key}")
    _same(_binding(validation["response"], "validation.response"), accounting_file_binding(packet["response"]), "validated response")
    sample = _document(packet["sample"], "successful sample")
    _same(_binding(validation["sample"], "validation.sample"), accounting_file_binding(packet["sample"]), "validated sample")
    _header(sample, "sample")
    coordinates = {"attempt_id": identity["attempt_id"], "commit": plan["commit"],
                   "hardware_sha256": plan["hardware"]["sha256"],
                   "hardware_profile_sha256": plan["hardware"]["profile_sha256"],
                   **{key: job[key] for key in ("profile", "participants", "seed", "run", "warmup", "configuration_sha256")}}
    for key, expected in coordinates.items():
        _same(sample.get(key), expected, f"sample.{key}")
    payload = result["payload"]
    if not isinstance(payload, dict) or not isinstance(payload.get("stages_ms"), dict):
        raise AccountingError("successful result lacks its measurement payload")
    metric_fields = frozenset({"stages_ms", "throughput_bundles_per_second", "cpu_seconds", "peak_rss_bytes",
                               "network_bytes", "proof_bytes", "receipt_bytes", "storage_growth_bytes"})
    exact_fields(sample, frozenset({"version", "protocol", *coordinates, *metric_fields}), "sample")
    # The canonical runner normalizes every measured stage/resource to f64.
    def normalized(value: Any) -> float:
        if type(value) not in (int, float) or value < 0:
            raise AccountingError("successful measurement is not a nonnegative number")
        result = float(value)
        accounting_canonical_bytes(result)
        return result
    expected_metrics = {key: normalized(payload[key]) for key in metric_fields if key != "stages_ms"}
    expected_metrics["stages_ms"] = {key: normalized(value) for key, value in payload["stages_ms"].items()}
    for key, expected in expected_metrics.items():
        _same(sample[key], expected, f"normalized sample.{key}")
    return sample


def _reduce_attempt(packet: Mapping[str, Any], *, scope_sha: str, campaign_id: str, plan_sha: str,
                    plan: Mapping[str, Any], job: Mapping[str, Any], ordinal: int, closure: Mapping[str, Any],
                    registered_ns: int, policy: Mapping[str, Any], nonces: set[str]) -> tuple[dict[str, Any], dict[str, Any] | None]:
    exact_fields(packet, PACKET_FIELDS, "attempt packet")
    attempt_id = registered_attempt_id(scope_sha, campaign_id, plan_sha, job["request_id"])
    identity = {"scope_sha256": scope_sha, "campaign_id": campaign_id, "plan_sha256": plan_sha,
                "attempt_id": attempt_id, "request_id": job["request_id"]}
    row = {**identity, **{key: job[key] for key in ("profile", "participants", "warmup")}}
    def finish(state: str, reason: str, sample: dict[str, Any] | None = None):
        return {**row, "state": state, "reason": reason}, sample
    if packet["started"] is None:
        if any(packet[key] is not None for key in PACKET_FIELDS - {"request_id", "request", "started"}):
            raise AccountingError("attempt evidence exists without a durable start")
        if packet["request"] is not None:
            _document(packet["request"], "unstarted request")
        return finish("not_started", "closed_without_durable_start")
    if packet["request"] is None:
        raise AccountingError("durable start lacks its bound request bytes")
    start = exact_fields(_document(packet["started"], "start"), START_FIELDS, "start")
    identity["invocation_nonce"] = _digest(start["invocation_nonce"], "invocation nonce")
    if identity["invocation_nonce"] in nonces:
        raise AccountingError("an invocation nonce is reused within the registered scope")
    nonces.add(identity["invocation_nonce"])
    _record_identity(start, identity, "start")
    started_ns = unsigned_milliseconds(start["started_ns"], "start time", positive=True)
    if not registered_ns <= started_ns <= closure["closed_ns"]:
        raise AccountingError("dispatch precedes registration or follows campaign closure")
    if (type(start["timeout_seconds"]) is not int or start["timeout_seconds"] * 1_000 != policy["outer_timeout_ms"]
            or not isinstance(start["command"], list) or not start["command"]
            or not all(isinstance(value, str) for value in start["command"])):
        raise AccountingError("durable start differs from the frozen invocation contract")
    request = _document(packet["request"], "request")
    _request_join(request, start, plan, job, packet["request"])
    process = None if packet["process"] is None else _process_record(
        _document(packet["process"], "process"), identity, start, closure["closed_ns"], policy)
    validation = None if packet["validation"] is None else _validation_record(
        _document(packet["validation"], "validation"), identity, ordinal, start, closure["closed_ns"])
    if process is not None and validation is not None and validation["finished_ns"] < process["finished_ns"]:
        raise AccountingError("semantic validation precedes runner process completion")
    response_record = _response_record(packet["response_outcome"], packet["response"])
    adapter = None if packet["adapter"] is None else validate_adapter_outcome(
        _document(packet["adapter"], "adapter"), request=request, request_sha256=start["request"]["sha256"])
    terminal = None
    if packet["rust_terminal"] is not None:
        if adapter is None:
            _terminal_without_exit(_document(packet["rust_terminal"], "Rust terminal"), request, start["request"]["sha256"])
        else:
            if adapter["rust_terminal"] is None:
                raise AccountingError("adapter denies a retained Rust terminal")
            _same(adapter["rust_terminal"], accounting_file_binding(packet["rust_terminal"]), "Rust terminal binding")
            terminal = validate_benchmark_terminal(_document(packet["rust_terminal"], "Rust terminal"),
                request=request, request_sha256=start["request"]["sha256"], exit_code=adapter["exit_code"])
            if adapter["elapsed_ms"] < terminal["elapsed_ms"]:
                raise AccountingError("adapter duration is shorter than its Rust worker")
            if terminal["outcome"]["kind"] == "timed_out":
                outcome = terminal["outcome"]
                if outcome["budget_ms"] != policy["rust_deadline_budgets_ms"][outcome["stage"]]:
                    raise AccountingError("Rust deadline differs from the registered budget")
    if adapter is not None and adapter["status"] == "invalid":
        raise AccountingError("adapter rejected the retained transport or measurement")
    if process is None:
        return finish("incomplete", "process_terminal_missing")
    if adapter is not None and adapter["elapsed_ms"] > process["elapsed_ms"]:
        raise AccountingError("adapter duration exceeds the observed runner invocation")
    if not process["owned_process_group_gone"]:
        return finish("incomplete", "owned_processes_unfinished")
    if process["completion_kind"] == "outer_deadline":
        if validation is not None and validation["validation_kind"] == "accepted":
            raise AccountingError("outer deadline contradicts completed semantic validation")
        return finish("timed_out", "outer_deadline")
    if process["completion_kind"] == "interrupted":
        return finish("incomplete", "runner_interrupted")
    if process["completion_kind"] == "spawn_failed":
        if adapter is not None or terminal is not None or packet["response"] is not None or response_record is not None:
            raise AccountingError("spawn failure contradicts downstream process evidence")
        return finish("failed", "harness_spawn_failed")
    if validation is not None and validation["validation_kind"] == "interrupted":
        return finish("incomplete", "validation_interrupted")
    if adapter is None:
        return finish("incomplete", "adapter_terminal_missing")
    if adapter["rust_terminal"] is not None and packet["rust_terminal"] is None:
        if validation is not None and validation["validation_kind"] == "accepted":
            raise AccountingError("accepted validation omitted its bound Rust terminal")
        return finish("incomplete", "rust_terminal_missing")
    if adapter["status"] == "incomplete":
        return finish("incomplete", adapter["reason"])
    if adapter["status"] in {"failed", "timed_out"}:
        if process["exit_code"] == 0 or process["passed"] or (response_record is not None and response_record["passed"]):
            raise AccountingError("unsuccessful transport contradicts successful runner completion")
        if validation is not None and validation["validation_kind"] == "accepted":
            raise AccountingError("unsuccessful transport has accepted validation")
        if adapter["reason"] in {"rust_failed", "rust_timed_out"}:
            if terminal is None or terminal["outcome"]["kind"] != adapter["status"]:
                raise AccountingError("adapter cause disagrees with the typed Rust outcome")
        elif terminal is not None:
            raise AccountingError("pre-worker failure carries a Rust terminal")
        return finish(adapter["status"], adapter["reason"])
    if terminal is None or terminal["outcome"]["kind"] != "succeeded" or process["exit_code"] != 0:
        raise AccountingError("successful transport contradicts worker or runner completion")
    if not process["passed"]:
        return finish("incomplete", "runner_completion_unaccepted")
    if validation is None or validation["validation_kind"] == "not_validated" or response_record is None:
        return finish("incomplete", "semantic_validation_missing")
    if validation["validation_kind"] == "publication_failed":
        return finish("failed", "publication_failed")
    if not response_record["passed"]:
        raise AccountingError("successful transport has a rejected response")
    if packet["sample"] is None or packet["response"] is None:
        raise AccountingError("accepted validation omitted bound successful bytes")
    sample = _successful_join(packet, request, terminal["outcome"]["result"], validation, identity, plan, job)
    return finish("succeeded", "validated_measurement", sample)


def reduce_registered_scope(scope_raw: bytes, campaigns: Sequence[Mapping[str, Any]],
                            successful_rows: Sequence[bytes]) -> dict[str, Any]:
    """Fold a complete registered, quiescent scope into public benchmark counts.

    Packets contain original retained JSON bytes (or explicit None), not trusted
    counters. File authentication and source/measurement qualification remain
    caller responsibilities. This function never reads files or launches work.
    Every campaign, planned benchmark job, and successful measurement must occur
    exactly once. Missing authoritative terminals remain incomplete; absence of
    a quiescent closure cannot establish a not-started denominator.
    """

    scope = exact_fields(_document(scope_raw, "scope"), SCOPE_FIELDS, "scope")
    _header(scope, "scope")
    scope_binding = accounting_file_binding(scope_raw)
    scope_sha = scope_binding["sha256"]
    _digest(scope["scope_id"], "scope_id")
    if scope["previous_scope_sha256"] is not None:
        raise AccountingError("V1 requires the complete scope upfront, without an unauthenticated predecessor")
    registered_ns = unsigned_milliseconds(scope["registered_ns"], "registration time", positive=True)
    if scope["stopping_policy"] != "fail_fast":
        raise AccountingError("scope has an undeclared stopping policy")
    policy = validate_deadline_policy(scope["deadline_policy"])
    slots = scope["campaigns"]
    if not isinstance(slots, list) or not slots or not isinstance(campaigns, (list, tuple)):
        raise AccountingError("scope must declare its complete ordered campaign inventory")
    ids = []
    for slot in slots:
        exact_fields(slot, frozenset({"campaign_id", "plan"}), "campaign registration")
        ids.append(_campaign_id(slot["campaign_id"]))
        _binding(slot["plan"], "registered plan")
    if ids != sorted(set(ids)) or any(not isinstance(packet, dict) for packet in campaigns) or [packet.get("campaign_id") for packet in campaigns] != ids:
        raise AccountingError("campaign inventory omits, duplicates or reorders a registered slot")
    rows, expected_samples, summaries, nonces = [], {}, [], set()
    campaign_environment = None
    for slot, packet in zip(slots, campaigns):
        exact_fields(packet, frozenset({"campaign_id", "plan", "closure", "attempts"}), "campaign packet")
        _same(accounting_file_binding(packet["plan"]), slot["plan"], "registered plan bytes")
        plan = exact_fields(_document(packet["plan"], "plan"), PLAN_FIELDS, "plan")
        _header(plan, "plan")
        _same(validate_deadline_policy(plan["benchmark_accounting"]), policy, "plan accounting policy")
        if (not isinstance(plan["commit"], str) or re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", plan["commit"]) is None
                or plan["worktree_clean"] is not True or plan["publication_evidence"] is not False or plan["execution_required"] is not True):
            raise AccountingError("registered plan has an invalid source/execution header")
        _binding(plan["harness"], "plan harness")
        if not isinstance(plan["hardware"], dict):
            raise AccountingError("plan hardware binding is absent")
        for key in ("sha256", "profile_sha256"):
            _digest(plan["hardware"].get(key), f"hardware.{key}")
        environment = {key: plan[key] for key in ("commit", "hardware", "benchmark_accounting")}
        if campaign_environment is None:
            campaign_environment = environment
        else:
            _same(environment, campaign_environment, "scope campaign source/hardware/deadline identity")
        closure = exact_fields(_document(packet["closure"], "closure"), CLOSURE_FIELDS, "closure")
        _record_identity(closure, {"scope_sha256": scope_sha, "campaign_id": slot["campaign_id"],
                                  "plan_sha256": slot["plan"]["sha256"]}, "closure")
        closed_ns = unsigned_milliseconds(closure["closed_ns"], "closure time", positive=True)
        if (closure["quiescent"] is not True or closed_ns < registered_ns
                or closure["reason"] not in {"completed", "fail_fast", "preparation_failed", "recovered_interruption", "not_run"}):
            raise AccountingError("campaign lacks an authoritative quiescent closure")
        jobs = plan["jobs"]
        if not isinstance(jobs, list) or not jobs:
            raise AccountingError("plan job inventory is empty or malformed")
        job_ids, benchmark_jobs = [], []
        for ordinal, job in enumerate(jobs, 1):
            if not isinstance(job, dict) or "request_id" not in job:
                raise AccountingError("plan job is malformed")
            job_id = _digest(job["request_id"], "job request_id")
            if hashlib.sha256(accounting_canonical_bytes({key: value for key, value in job.items() if key != "request_id"})).hexdigest() != job_id:
                raise AccountingError("job request identity does not bind its exact planned coordinates")
            job_ids.append(job_id)
            if job.get("kind") == "benchmark":
                exact_fields(job, BENCHMARK_JOB_FIELDS, "benchmark job")
                if (job["profile"] not in {"private", "transparent_control"} or type(job["participants"]) is not int
                        or job["participants"] not in (2, 3, 4, 8, 16) or type(job["warmup"]) is not bool):
                    raise AccountingError("benchmark cohort is malformed")
                _digest(job["configuration_sha256"], "job configuration")
                unsigned_milliseconds(job["seed"], "job seed")
                unsigned_milliseconds(job["run"], "job run")
                benchmark_jobs.append((ordinal, job))
            elif job.get("kind") not in {"fault", "leakage"}:
                raise AccountingError("plan contains an unknown job kind")
        started_ids = closure["started_request_ids"]
        if (len(job_ids) != len(set(job_ids)) or not isinstance(started_ids, list)
                or not all(isinstance(value, str) for value in started_ids)
                or len(started_ids) != len(set(started_ids)) or not set(started_ids) <= set(job_ids)):
            raise AccountingError("closure has a duplicate or unplanned durable start")
        if started_ids != job_ids[:len(started_ids)]:
            raise AccountingError("fail-fast campaign skipped an earlier planned dispatch")
        attempts = packet["attempts"]
        if (not isinstance(attempts, list) or any(not isinstance(item, dict) for item in attempts)
                or [item.get("request_id") for item in attempts] != [job["request_id"] for _, job in benchmark_jobs]):
            raise AccountingError("campaign omits, duplicates or reorders a planned benchmark job")
        actual_benchmark_starts = [item["request_id"] for item in attempts if item.get("started") is not None]
        if actual_benchmark_starts != [value for value in started_ids if value in {job["request_id"] for _, job in benchmark_jobs}]:
            raise AccountingError("closure inventory differs from retained durable starts")
        campaign_rows = []
        stopped = False
        for (ordinal, job), attempt in zip(benchmark_jobs, attempts):
            if stopped and attempt["started"] is not None:
                raise AccountingError("campaign dispatched a later benchmark after fail-fast termination")
            row, sample = _reduce_attempt(attempt, scope_sha=scope_sha, campaign_id=slot["campaign_id"],
                plan_sha=slot["plan"]["sha256"], plan=plan, job=job, ordinal=ordinal, closure=closure,
                registered_ns=registered_ns, policy=policy, nonces=nonces)
            if row["state"] != "succeeded" and len(started_ids) > ordinal:
                raise AccountingError("campaign dispatched a later full-plan job after fail-fast termination")
            campaign_rows.append(row)
            rows.append(row)
            if sample is not None:
                if row["attempt_id"] in expected_samples:
                    raise AccountingError("a planned attempt was reused")
                expected_samples[row["attempt_id"]] = sample
            if row["state"] in {"failed", "timed_out", "not_started"}:
                stopped = True
        if closure["reason"] == "completed" and (len(started_ids) != len(jobs) or any(row["state"] != "succeeded" for row in campaign_rows)):
            raise AccountingError("completed campaign has unstarted or unsuccessful jobs")
        if closure["reason"] == "not_run" and started_ids:
            raise AccountingError("unused campaign contains a durable start")
        summaries.append({"campaign_id": slot["campaign_id"], "plan_sha256": slot["plan"]["sha256"],
                          "counts": _counter(campaign_rows)})
    observed_samples = {}
    for raw in successful_rows:
        sample = _document(raw, "published sample")
        attempt_id = _digest(sample.get("attempt_id"), "sample attempt_id")
        if attempt_id in observed_samples or attempt_id not in expected_samples:
            raise AccountingError("published sample is duplicate, unplanned or unsuccessful")
        _same(sample, expected_samples[attempt_id], "published successful row")
        observed_samples[attempt_id] = sample
    if set(observed_samples) != set(expected_samples):
        raise AccountingError("a retained successful measurement was omitted")
    cohorts = []
    for profile, participants, warmup in sorted({(row["profile"], row["participants"], row["warmup"]) for row in rows}):
        cohort = [row for row in rows if (row["profile"], row["participants"], row["warmup"]) == (profile, participants, warmup)]
        cohorts.append({"profile": profile, "participants": participants, "warmup": warmup, "counts": _counter(cohort)})
    return {"version": VERSION, "protocol": PROTOCOL, "scope_id": scope["scope_id"], "scope": scope_binding,
            "previous_scope_sha256": scope["previous_scope_sha256"], "timeout_scope": TIMEOUT_SCOPE,
            "deadline_policy": policy, "counts": _counter(rows), "cohorts": cohorts, "campaigns": summaries,
            "rows": rows, "accounting_complete": all(row["state"] != "incomplete" for row in rows)}


if __name__ == "__main__":
    argparse.ArgumentParser(description=__doc__).parse_args()
