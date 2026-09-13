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


# The canonical runner uses this plan with retained process ownership and the
# mandatory session evidence contract; planning alone grants no execution claim.
BENCHMARK_SESSION_PARTICIPANTS = (2, 3, 4, 8, 16)
BENCHMARK_SESSION_PROFILES = ("private", "transparent_control")
# Preserve the existing planner's maximum benchmark inventory before allocation.
MAX_PLANNED_BENCHMARK_ATTEMPTS = 20_000


def build_benchmark_workload_policy(participants: int) -> dict[str, Any]:
    """Declare the exact shared payment policy; no private party IDs are exported."""
    if type(participants) is not int or participants not in BENCHMARK_SESSION_PARTICIPANTS:
        raise AccountingError("workload policy has an unsupported participant count")
    return {
        "version": 1, "protocol": PROTOCOL, "kind": "matched_benchmark_payment_policy",
        "participants": participants, "primary_amount_base": 42, "primary_amount_step": 1,
        "sponsor_reimbursement_amount": 5, "private_change_amount": 7, "reserve_note_amount": 1,
        "sponsor": "genesis_alice", "derivation_domain": "iroha:matched-benchmark-workload:v1",
        "attempt_coordinates": ["participants", "seed", "session_attempt_index", "warmup"],
        "prefunding_policy": "session_union_of_disjoint_attempts",
    }


def validate_benchmark_workload_policy(value: Any, participants: int) -> dict[str, Any]:
    """Reject policy substitution, including omitted mandatory reimbursement."""
    expected = build_benchmark_workload_policy(participants)
    _same(value, expected, "matched economic policy")
    return expected

def build_benchmark_session_plan(
    configuration_sha256: Mapping[int, str], seeds: Sequence[int],
    workload_manifest_sha256: Mapping[int, str], *, warmups_per_session: int,
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
    for label, value in (("configuration", configuration_sha256), ("workload", workload_manifest_sha256)):
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
                    "workload_manifest_sha256": workload_manifest_sha256[participants],
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
                        "workload_manifest_sha256": workload_manifest_sha256[participants],
                    }
                    jobs.append({"request_id": hashlib.sha256(accounting_canonical_bytes(body)).hexdigest(), **body})
    return {"sessions": sessions, "jobs": jobs}


def validate_benchmark_session_plan(
    value: Any, configuration_sha256: Mapping[int, str], seeds: Sequence[int],
    workload_manifest_sha256: Mapping[int, str], *, warmups_per_session: int,
    measured_per_profile: int,
) -> dict[str, list[dict[str, Any]]]:
    """Require the exact regenerated session and attempt inventory, not counters."""
    expected = build_benchmark_session_plan(
        configuration_sha256, seeds, workload_manifest_sha256,
        warmups_per_session=warmups_per_session, measured_per_profile=measured_per_profile)
    _same(value, expected, "canonical benchmark session plan")
    return expected


def validate_benchmark_session_dispatch_prefix(
    configuration_sha256: Mapping[int, str], seeds: Sequence[int],
    workload_manifest_sha256: Mapping[int, str], *, warmups_per_session: int,
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
        configuration_sha256, seeds, workload_manifest_sha256,
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
    """Validate an attempt terminal without inventing a per-attempt process exit."""

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


# A retained benchmark has one NativeWorker lifetime. The in-process adapter
# does not acquire a fictitious child exit, and attempts do not own process groups.
SESSION_CLOSURE_FIELDS = frozenset({
    "session_started", "worker_terminal", "adapter_lifecycle", "ready",
    "process_observation", "closed_ns", "bindings_unchanged", "adapter_thread_joined",
})


class _SessionRecords:
    """Consume a mandatory authenticated lazy provider, with no raw-dict fallback."""

    def __init__(self, provider):
        import private_settlement_session_control as control
        if not callable(getattr(provider, "inventory", None)) or not callable(getattr(provider, "read", None)):
            raise AccountingError("session records require an authenticated inventory/read provider")
        self.control, self.provider = control, provider
        inventory = provider.inventory()
        if type(inventory) is not dict:
            raise AccountingError("record provider has no complete path inventory")
        self.inventory = {}
        for path, reference in inventory.items():
            self._reference(reference)
            if path != reference["path"]:
                raise AccountingError("inventory key differs from its located record")
            self.inventory[path] = dict(reference)

    def _reference(self, reference):
        exact_fields(reference, frozenset({"path", "sha256", "bytes"}), "physical file binding")
        # Physical inventory includes empty/large opaque logs. Protocol record
        # consumers independently enforce control.reference's nonempty limit.
        self.control.reference({**reference, "bytes": 1})
        unsigned_milliseconds(reference["bytes"], "physical file bytes")

    def locate(self, path):
        if path not in self.inventory:
            raise AccountingError("required retained session record is absent")
        return dict(self.inventory[path])

    def read(self, reference):
        self._reference(reference)
        _same(self.locate(reference["path"]), reference, "retained record locator and bytes")
        raw = self.provider.read(reference)
        if (type(raw) is not bytes or len(raw) != reference["bytes"]
                or hashlib.sha256(raw).hexdigest() != reference["sha256"]):
            raise AccountingError("lazy provider returned substituted or shortened record bytes")
        return raw

    def validate(self):
        """Require the final complete physical inventory to match its initial cut."""
        _same(self.provider.inventory(), self.inventory, "physical record inventory changed during reduction")

    def object(self, reference):
        return _document(self.read(reference), "retained session record")


def _session_chain_inventory(records, identity, started):
    """Replay each actual endpoint journal, including unconsumed sender suffixes."""
    control = records.control
    prefix = f"sessions/{identity['session_id']}/control/"
    pattern = re.compile(r"(runner|adapter|worker)\.(runner_adapter|adapter_worker)\."
                         r"(owner_to_child|child_to_owner)\.([0-9]{20})\.frame")
    chains = {}
    for path in records.inventory:
        if not path.startswith(prefix) or not path.endswith(".frame"):
            continue
        match = pattern.fullmatch(path[len(prefix):])
        if match is None:
            raise AccountingError("control journal has an unknown owner or filename")
        observer, channel, direction, ordinal = match.groups()
        key = observer, channel, direction
        chains.setdefault(key, []).append((int(ordinal), path))
    frames = {}
    for (observer, channel, direction), rows in chains.items():
        chain = control.ControlChain(identity, started["sha256"], channel, direction, records,
            observer=observer, journal_prefix=prefix[:-1])
        observed = []
        for ordinal, path in sorted(rows):
            if ordinal != len(observed):
                raise AccountingError("control journal omits or repeats a sequence")
            wire = control.RetainedMessage(records.read(records.locate(path)), records.locate(path))
            chain._check(wire.decoded())
            chain._advance(wire.raw)
            observed.append(wire)
            frames[path] = wire
        chains[observer, channel, direction] = observed
    for channel in control.CHANNELS:
        owner, child = control.CHANNEL_ENDPOINTS[channel]
        for direction in control.DIRECTIONS:
            sender, receiver = (owner, child) if direction == "owner_to_child" else (child, owner)
            sent = chains.get((sender, channel, direction), [])
            received = chains.get((receiver, channel, direction), [])
            if len(received) > len(sent) or any(a.raw != b.raw for a, b in zip(sent, received)):
                raise AccountingError("receiver journal differs from the sender's exact prefix")
    for wire in frames.values():
        reference = wire.decoded()["forwarded_from"]
        if reference is not None:
            upstream = control.RetainedMessage(records.read(reference), reference)
            control.verify_forwarded(wire, upstream)
    return chains


def _session_lifetime(records, closure, identity, started, accepted, active, expected_worker, *, unready_attempts_absent):
    """Join the one real worker to its birth inventory, natural wait and absence."""
    control = records.control
    lifecycle = records.object(closure["adapter_lifecycle"])
    fields = control.IDENTITY_FIELDS | {
        "kind", "worker_terminal", "worker_process_start", "worker_pid", "worker_exit_code",
        "worker_wait_completed", "group_before", "kernel_absences", "group_after",
        "worker_sha256", "worker_image_unchanged",
    }
    exact_fields(lifecycle, fields, "session worker lifecycle")
    _record_identity(lifecycle, identity, "session worker lifecycle")
    if (lifecycle["kind"] != "benchmark_session_worker_lifecycle"
            or lifecycle["worker_terminal"] != closure["worker_terminal"]
            or lifecycle["worker_wait_completed"] is not True
            or lifecycle["worker_image_unchanged"] is not True
            or lifecycle["worker_sha256"] != expected_worker["sha256"]
            or type(lifecycle["worker_exit_code"]) is not int):
        raise AccountingError("session has no exact completed native worker wait")
    _same(lifecycle["worker_process_start"]["path"], f"sessions/{identity['session_id']}/worker-process-start.json", "native process start path")
    process = records.object(lifecycle["worker_process_start"])
    exact_fields(process, frozenset({"command", "pid", "parent_pid", "process_group",
                                    "started_ns", "worker_sha256"}), "worker process start")
    pid = process["pid"]
    if (type(pid) is not int or not 1 < pid < 1 << 31
            or lifecycle["worker_pid"] != pid or process["process_group"] != pid
            or type(process["parent_pid"]) is not int or process["parent_pid"] <= 1
            or process["worker_sha256"] != expected_worker["sha256"]
            or process["command"] != started["command"]
            or not started["started_ns"] <= unsigned_milliseconds(process["started_ns"], "worker start", positive=True) <= closure["closed_ns"]):
        raise AccountingError("worker process start differs from its unique session owner")
    terminal = records.object(closure["worker_terminal"])
    exact_fields(terminal, control.IDENTITY_FIELDS | {
        "kind", "reason", "accepted_request_ids", "active_attempt_id", "last_owner_message_sha256",
        "last_worker_message_sha256", "network_shutdown_observed", "coordinator_reaped_observed",
    }, "session worker terminal")
    _record_identity(terminal, identity, "session worker terminal")
    unready_setup_failure = (unready_attempts_absent and closure["ready"] is None
        and closure["process_observation"] is None and terminal["kind"] == "failed"
        and terminal["reason"] == "setup_failed" and accepted == [] and active is None
        and terminal["network_shutdown_observed"] is False
        and terminal["coordinator_reaped_observed"] is False)
    cleanup_observed = (terminal["network_shutdown_observed"] is True
                        and terminal["coordinator_reaped_observed"] is True)
    if (terminal["kind"] not in {"completed", "failed", "timed_out", "incomplete"}
            or terminal["accepted_request_ids"] != accepted or terminal["active_attempt_id"] != active
            or not (cleanup_observed or unready_setup_failure)
            or (lifecycle["worker_exit_code"] == 0) != (terminal["kind"] == "completed")
            or (terminal["reason"] is not None if terminal["kind"] == "completed"
                else terminal["reason"] not in control.STOP_REASONS)):
        raise AccountingError("session terminal contradicts accepted attempts or observed cleanup")
    pids = {pid}
    worker_generation = None
    if closure["ready"] is not None:
        ready = records.object(closure["ready"])
        exact_fields(ready, control.IDENTITY_FIELDS | {"network_id", "genesis_sha256", "configuration_sha256",
            "workload_manifest_sha256", "activated_height", "process_inventory", "worker_pid", "network_ports"}, "session ready")
        _record_identity(ready, identity, "session ready")
        requested = records.object(started["request"])
        if (ready["worker_pid"] != pid or type(ready["activated_height"]) is not int or ready["activated_height"] < 301
                or any(ready[key] != requested[key] for key in ("configuration_sha256", "workload_manifest_sha256"))):
            raise AccountingError("ready belongs to a different worker")
        _digest(ready["genesis_sha256"], "native genesis")
        _same(ready["network_ports"]["path"], f"sessions/{identity['session_id']}/network-ports.json", "session port manifest")
        records.read(ready["network_ports"])
        observed = records.object(closure["process_observation"])
        exact_fields(observed, control.IDENTITY_FIELDS | {"kind", "ready", "process_scope", "listeners", "network_ports"}, "session kernel readiness")
        _record_identity(observed, identity, "session process observation")
        if (observed["ready"] != closure["ready"] or observed["kind"] != "benchmark_session_process_ready"
                or observed["network_ports"] != ready["network_ports"] or type(observed["listeners"]) is not dict):
            raise AccountingError("kernel inventory does not bind the actual ready record")
        processes = observed["process_scope"]["processes"]
        if type(processes) is not list or not processes:
            raise AccountingError("session has no admitted kernel generation inventory")
        declarations = ready["process_inventory"]
        if (type(declarations) is not list or len(declarations) != (requested["participants"]+1)*4+1
                or len({row["pid"] for row in declarations}) != len(declarations)):
            raise AccountingError("native readiness omits a validator/coordinator owner")
        declared = {row["pid"] for row in declarations} | {pid}
        identities = [row["identity"] for row in processes]
        if len({row["pid"] for row in identities}) != len(identities) or {row["pid"] for row in identities} != declared:
            raise AccountingError("kernel inventory omits or repeats a declared session process")
        for value in identities:
            if type(value.get("birth")) is not dict or not value["birth"] or value.get("pgid") != pid:
                raise AccountingError("session process lacks its exact birth or owned group")
            birth = value["birth"]
            if birth.get("kind") == "darwin_bsdinfo":
                exact_fields(birth, frozenset({"kind", "started_seconds", "started_microseconds", "start_abstime"}), "Darwin birth")
                for key in ("started_seconds", "start_abstime"):
                    unsigned_milliseconds(birth[key], "native birth", positive=True)
                if type(birth["started_microseconds"]) is not int or not 0 <= birth["started_microseconds"] < 1_000_000:
                    raise AccountingError("native microsecond birth is malformed")
                if re.fullmatch(r"[0-9a-f]{32}", value.get("loaded_image_uuid", "")) is None:
                    raise AccountingError("native loaded image identity is missing")
            elif birth.get("kind") == "linux_proc":
                exact_fields(birth, frozenset({"kind", "boot_id", "start_ticks"}), "Linux birth")
                unsigned_milliseconds(birth["start_ticks"], "native birth", positive=True)
                if re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", birth["boot_id"]) is None:
                    raise AccountingError("native boot identity is malformed")
            else:
                raise AccountingError("unsupported native birth owner")
            if value["pid"] == pid and (value.get("executable_sha256") != expected_worker["sha256"]
                                       or value.get("ppid") != process["parent_pid"]):
                raise AccountingError("admitted worker generation differs from actual process start")
            if value["pid"] == pid:
                worker_generation = hashlib.sha256(accounting_canonical_bytes(value)).hexdigest()
        pids = declared
    elif closure["process_observation"] is not None:
        raise AccountingError("kernel readiness exists without native readiness")
    for group in (lifecycle["group_before"], lifecycle["group_after"]):
        exact_fields(group, frozenset({"process_group", "members", "utility_sha256", "observed_monotonic_ns"}), "group absence")
        if group["process_group"] != pid or group["members"] != []:
            raise AccountingError("session still owns a process group")
        _digest(group["utility_sha256"], "group observation utility")
    _same(lifecycle["group_before"]["utility_sha256"], lifecycle["group_after"]["utility_sha256"], "group observation utility")
    previous = unsigned_milliseconds(lifecycle["group_before"]["observed_monotonic_ns"], "group observation", positive=True)
    absences = lifecycle["kernel_absences"]
    if type(absences) is not list or [row.get("pid") for row in absences] != sorted(pids):
        raise AccountingError("session closure omits an owned process absence")
    for row in absences:
        exact_fields(row, frozenset({"pid", "kernel_absence_observed", "observed_monotonic_ns"}), "kernel absence")
        stamp = unsigned_milliseconds(row["observed_monotonic_ns"], "kernel absence time", positive=True)
        if row["kernel_absence_observed"] is not True or stamp < previous:
            raise AccountingError("process absence is unobserved or outside its bracket")
        previous = stamp
    if unsigned_milliseconds(lifecycle["group_after"]["observed_monotonic_ns"], "group observation", positive=True) < previous:
        raise AccountingError("kernel absence observations have a reversed bracket")
    return terminal, worker_generation


def _ordered_session_attempt_controls(frames):
    """Require the one-active-attempt dispatch/accept order on an owner chain."""
    import private_settlement_session_control as control
    kinds = [wire.decoded()["kind"] for wire in frames
             if wire.decoded()["kind"] not in control.LOCAL_MEASUREMENT_KINDS]
    if kinds and kinds[-1] == "stop": kinds = kinds[:-1]
    if kinds != ["dispatch", "accept"] * (len(kinds)//2) + (["dispatch"] if len(kinds)%2 else []):
        raise AccountingError("session owner reused an active attempt or reordered dispatch/accept")


def reduce_retained_session(descriptor, packet, *, records, campaign_identity, jobs,
                            registered_ns, closed_ns, policy, worker_command, worker_image,
                            validate_success):
    """Reduce one retained network, with no per-attempt process-exit fiction.

    The collector authenticates complete record inventories and native process
    observations. validate_success must independently replay the existing native
    economics/resource/packet owners and return exact recomputed sample bytes.
    This pure boundary cannot substitute a claimed metric or a supplied count.
    """
    if not callable(validate_success):
        raise AccountingError("retained samples require their canonical semantic replay owner")
    control = records.control
    sid = _digest(descriptor["session_id"], "session id")
    indexed = [(ordinal, job) for ordinal, job in jobs if job["session_id"] == sid]
    coordinates = {key: descriptor[key] for key in ("profile", "participants", "seed",
        "configuration_sha256", "workload_manifest_sha256", "warmup_attempts", "measured_attempts")}
    _same(sid, hashlib.sha256(accounting_canonical_bytes({
        "domain": "iroha:private-settlement:benchmark-session-plan:v1", **coordinates})).hexdigest(), "session plan identity")
    if len(indexed) != descriptor["warmup_attempts"] + descriptor["measured_attempts"]:
        raise AccountingError("session planned attempt count differs")
    rows = [{**campaign_identity, "attempt_id": registered_attempt_id(
                campaign_identity["scope_sha256"], campaign_identity["campaign_id"], campaign_identity["plan_sha256"], job["request_id"]),
             **{key: job[key] for key in ("request_id", "profile", "participants", "warmup", "session_id", "session_attempt_index")},
             "state": "not_started", "reason": "closed_without_durable_start"} for _, job in indexed]
    if packet is None:
        if any(path.startswith(f"sessions/{sid}/") and path.endswith("started.json") for path in records.inventory):
            raise AccountingError("unstarted session contains an owner start")
        for ordinal, job in indexed:
            if f"attempts/{ordinal:05}-{job['request_id']}/started.json" in records.inventory:
                raise AccountingError("attempt start exists without its session owner")
        return rows, [], None
    exact_fields(packet, frozenset({"session_id", "closure"}), "retained session packet")
    _same(packet["session_id"], sid, "session packet")
    closure = records.object(packet["closure"])
    exact_fields(closure, control.IDENTITY_FIELDS | SESSION_CLOSURE_FIELDS, "retained session closure")
    identity = control.identity({key: closure[key] for key in control.IDENTITY_FIELDS})
    _record_identity(identity, {**campaign_identity, "session_id": sid}, "session closure")
    for key, leaf in (("session_started", "started.json"), ("worker_terminal", "worker-terminal.json"),
                      ("adapter_lifecycle", "adapter-lifecycle.json"), ("ready", "ready.json"),
                      ("process_observation", "process-ready.json")):
        if closure[key] is not None:
            _same(closure[key]["path"], f"sessions/{sid}/{leaf}", "session record path")
    if (closure["bindings_unchanged"] is not True or closure["adapter_thread_joined"] is not True
            or not registered_ns <= unsigned_milliseconds(closure["closed_ns"], "session cut", positive=True) <= closed_ns):
        raise AccountingError("session has no authoritative unchanged closure in the campaign cut")
    started = records.object(closure["session_started"])
    exact_fields(started, control.IDENTITY_FIELDS | {"request", "command", "harness", "started_ns"}, "session start")
    _record_identity(started, identity, "session start")
    _same(started["harness"], worker_image, "source-admitted native session executable")
    _same(started["command"], worker_command, "source-admitted native session command")
    if not registered_ns <= unsigned_milliseconds(started["started_ns"], "session start", positive=True) <= closure["closed_ns"]:
        raise AccountingError("session starts outside its registered cut")
    request = records.object(started["request"])
    _same(started["request"]["path"], f"sessions/{sid}/request.json", "session request path")
    if started["request"]["sha256"] != identity["session_request_sha256"]:
        raise AccountingError("session request generation changed")
    for key, value in {**campaign_identity, "session_id": sid,
        "session_invocation_nonce": identity["session_invocation_nonce"], "profile": descriptor["profile"],
        "participants": descriptor["participants"], "seed": descriptor["seed"],
        "configuration_sha256": descriptor["configuration_sha256"],
        "workload_manifest_sha256": descriptor["workload_manifest_sha256"], "warmups": descriptor["warmup_attempts"]}.items():
        _same(request.get(key), value, "session request coordinate")
    validate_benchmark_workload_policy(request["workload_manifest"], descriptor["participants"])
    _same(hashlib.sha256(accounting_canonical_bytes(request["workload_manifest"])).hexdigest(),
          descriptor["workload_manifest_sha256"], "session workload manifest")
    attempts = request["attempts"]
    if type(attempts) is not list or len(attempts) != len(indexed):
        raise AccountingError("session request omits planned attempts")
    chains = _session_chain_inventory(records, identity, closure["session_started"])
    ra_owner = chains.get(("runner", "runner_adapter", "owner_to_child"), [])
    ra_child = chains.get(("runner", "runner_adapter", "child_to_owner"), [])
    aw_owner = chains.get(("worker", "adapter_worker", "owner_to_child"), [])
    aw_child = chains.get(("worker", "adapter_worker", "child_to_owner"), [])
    _ordered_session_attempt_controls(ra_owner)
    _ordered_session_attempt_controls(aw_owner)
    by_kind = lambda frames, kind: [wire for wire in frames if wire.decoded()["kind"] == kind]
    dispatched, acknowledged = by_kind(ra_owner, "dispatch"), by_kind(ra_owner, "accept")
    worker_dispatched = by_kind(aw_owner, "dispatch")
    consumed = by_kind(aw_owner, "accept")
    completions = by_kind(ra_child, "attempt_completed")
    owner_kinds = [wire.decoded()["kind"] for wire in ra_owner]
    if any(kind not in {"dispatch", "accept", "stop"} for kind in owner_kinds) or (
        "stop" in owner_kinds and (owner_kinds.count("stop") != 1 or owner_kinds[-1] != "stop")):
        raise AccountingError("owner continued after stop or emitted an unexpected control")
    if [wire.decoded()["kind"] for wire in ra_child] != (
        (["ready"] if closure["ready"] is not None else []) + ["attempt_completed"] * len(completions) + ["session_completed"]):
        raise AccountingError("runner did not consume the ordered session completion chain")
    _same(ra_child[-1].decoded()["payload"], {"worker_terminal": closure["worker_terminal"],
        "adapter_lifecycle": closure["adapter_lifecycle"]}, "consumed session closure")
    if len(dispatched) > len(attempts) or len(consumed) > len(acknowledged):
        raise AccountingError("session control exceeds the planned attempts")
    if closure["ready"] is None and dispatched:
        raise AccountingError("attempt dispatch precedes native and kernel readiness")
    if closure["ready"] is not None:
        ready = by_kind(ra_child, "ready")
        if len(ready) != 1 or ready[0].decoded()["payload"] != {
            "ready": closure["ready"], "process_observation": closure["process_observation"]}:
            raise AccountingError("session ready is not the consumed authenticated control")
    samples, started_ids, nonces = [], [], set()
    for index, ((ordinal, job), attempt, row) in enumerate(zip(indexed, attempts, rows)):
        exact_fields(attempt, control.ATTEMPT_FIELDS | {"request", "output_directory"}, "planned session attempt")
        aid = {key: attempt[key] for key in control.ATTEMPT_FIELDS}
        control.attempt(aid)
        if (attempt["attempt_id"] != row["attempt_id"] or attempt["request_id"] != job["request_id"]
                or attempt["session_attempt_index"] != index or attempt["invocation_nonce"] in nonces
                or attempt["output_directory"] != f"attempts/{ordinal:05}-{job['request_id']}"):
            raise AccountingError("session attempt changes full-plan coordinates or generation")
        nonces.add(attempt["invocation_nonce"])
        raw_request = records.read(attempt["request"])
        single = _document(raw_request, "attempt request")
        for key, value in {**{key: aid[key] for key in aid if key != "attempt_id"},
                           "session_id": sid, "session_invocation_nonce": identity["session_invocation_nonce"]}.items():
            _same(single.get(key), value, "attempt request identity")
        for key in ("participants", "seed", "configuration_sha256", "workload_manifest_sha256"):
            _same(single.get(key), job[key], "attempt workload coordinates")
        _same(single.get("payload", {}).get("profile"), job["profile"], "attempt profile")
        _same(single.get("payload", {}).get("warmup"), job["warmup"], "attempt warmup")
        path = attempt["output_directory"] + "/started.json"
        if path not in records.inventory:
            if index < len(dispatched) or any(p.startswith(attempt["output_directory"] + "/")
                and p != attempt["request"]["path"] for p in records.inventory):
                raise AccountingError("attempt evidence exists without its durable start")
            continue
        start_ref = records.locate(path)
        start = records.object(start_ref)
        exact_fields(start, control.IDENTITY_FIELDS | control.ATTEMPT_FIELDS | {
            "ordinal", "session_started", "request", "outer_timeout_ms", "started_ns", "preceding_acceptance"}, "attempt start")
        _record_identity(start, {**identity, **aid}, "attempt start")
        if (start["ordinal"] != ordinal or start["session_started"] != closure["session_started"]
                or start["request"] != attempt["request"] or start["outer_timeout_ms"] != policy["outer_timeout_ms"]
                or type(start["ordinal"]) is not int or type(start["outer_timeout_ms"]) is not int
                or not started["started_ns"] <= unsigned_milliseconds(start["started_ns"], "attempt start", positive=True) <= closure["closed_ns"]):
            raise AccountingError("attempt start changes its owner, request, budget or cut")
        expected_previous = None if index == 0 else acknowledged[index-1].binding if index <= len(acknowledged) else None
        if start["preceding_acceptance"] != expected_previous or (index and index > len(acknowledged)):
            raise AccountingError("attempt start lacks its preceding runner acceptance")
        if index:
            predecessor = attempts[index-1]
            for phase in ("proposed", "validated", "pipe-written"):
                proof = records.object(records.locate(f"sessions/{sid}/control/ack-{index-1:06d}-{phase}.json"))
                _same(proof, {**identity, **{key: predecessor[key] for key in control.ATTEMPT_FIELDS},
                    "acknowledgement": acknowledged[index-1].binding, "phase": phase}, "predecessor acknowledgement barrier")
        if len(started_ids) != index:
            raise AccountingError("session start inventory is not the exact attempt prefix")
        started_ids.append(job["request_id"])
        row.update(state="incomplete", reason="attempt_terminal_missing")
        if index >= len(dispatched):
            continue
        _same(dispatched[index].decoded()["payload"], {**aid, "request": attempt["request"], "attempt_started": start_ref}, "dispatched attempt")
        if index >= len(completions):
            continue
        if index >= len(worker_dispatched) or (index and index > len(consumed)):
            raise AccountingError("native completion lacks worker dispatch and consumed predecessor")
        _same(worker_dispatched[index].decoded()["payload"], dispatched[index].decoded()["payload"], "worker received dispatch")
        completion = completions[index].decoded()["payload"]
        for key in control.ATTEMPT_FIELDS:
            _same(completion[key], aid[key], "completed attempt")
        _same(completion["rust_terminal"]["path"], attempt["output_directory"] + "/evidence/benchmark-protocol/rust-result.json", "native result path")
        _same(completion["adapter_outcome"]["path"], attempt["output_directory"] + "/evidence/benchmark-protocol/adapter-outcome.json", "adapter outcome path")
        native_raw = records.read(completion["rust_terminal"])
        native = _document(native_raw, "native attempt terminal")
        _terminal_without_exit(native, single, attempt["request"]["sha256"])
        kind = native["outcome"]["kind"]
        adapter = records.object(completion["adapter_outcome"])
        exact_fields(adapter, control.IDENTITY_FIELDS | control.ATTEMPT_FIELDS | {
            "request_sha256", "rust_terminal", "kind", "status", "measurement_window", "native_economic_verification", "response"}, "session adapter outcome")
        _record_identity(adapter, {**identity, **aid}, "session adapter outcome")
        if (adapter["kind"] != "benchmark_session_attempt_validation" or adapter["status"] != kind
                or adapter["request_sha256"] != attempt["request"]["sha256"]
                or adapter["rust_terminal"] != completion["rust_terminal"]
                or adapter["response"] != completion["response"]):
            raise AccountingError("adapter contradicts native attempt terminal")
        if kind != "succeeded":
            if index < len(acknowledged) or completion["response"] is not None:
                raise AccountingError("unsuccessful native attempt was accepted")
            if kind == "timed_out":
                timeout = native["outcome"]
                _same(timeout["budget_ms"], policy["rust_deadline_budgets_ms"][timeout["stage"]], "registered Rust deadline")
            row.update(state=kind, reason="typed_native_" + kind)
            continue
        validation_path = attempt["output_directory"] + "/validation-outcome.json"
        sample_path = attempt["output_directory"] + "/benchmark-sample.json"
        if validation_path not in records.inventory or sample_path not in records.inventory:
            row["reason"] = "semantic_acceptance_missing"
            continue
        payload = {**completion, "validation": records.locate(validation_path), "sample": records.locate(sample_path)}
        _same(payload["response"]["path"], attempt["output_directory"] + "/response.json", "native response path")
        ack = None if index >= len(acknowledged) else acknowledged[index]
        if ack is not None:
            _same(ack.decoded()["payload"], payload, "acceptance substituted a completed result")
        validation = records.object(payload["validation"])
        exact_fields(validation, control.IDENTITY_FIELDS | control.ATTEMPT_FIELDS | {
            "passed", "validation_kind", "response", "sample"}, "semantic acceptance")
        _record_identity(validation, {**identity, **aid}, "semantic acceptance")
        if validation["passed"] is not True or validation["validation_kind"] != "accepted" or any(
            validation[key] != payload[key] for key in ("response", "sample")):
            raise AccountingError("semantic acceptance contradicts its retained records")
        for phase in ("proposed", "validated", "pipe-written"):
            path = f"sessions/{sid}/control/ack-{index:06d}-{phase}.json"
            if path in records.inventory:
                if ack is None:
                    raise AccountingError("acknowledgement phase lacks its exact frame")
                proof = records.object(records.locate(path))
                _same(proof, {**identity, **aid, "acknowledgement": ack.binding, "phase": phase}, "acknowledgement publication phase")
        if index < len(consumed):
            _same(consumed[index].decoded()["payload"], payload, "worker consumed acceptance")
        bound = {key: records.read(payload[key]) for key in payload.keys() - control.ATTEMPT_FIELDS}
        computed = validate_success(identity, attempt, bound, records)
        if type(computed) is not bytes or computed != bound["sample"]:
            raise AccountingError("published sample differs from independently replayed measurements")
        sample = _document(computed, "accepted sample")
        _record_identity(sample, {**identity, **aid}, "accepted sample")
        for key in ("profile", "participants", "seed", "warmup", "configuration_sha256", "workload_manifest_sha256"):
            _same(sample.get(key), job[key], "accepted sample cohort")
        samples.append(computed)
        row.update(state="succeeded", reason="validated_measurement")
    if (len(dispatched) > len(started_ids) or len(worker_dispatched) > len(dispatched)
            or len(completions) > len(dispatched) or len(acknowledged) > len(completions)):
        raise AccountingError("session controls overrun their durable starts or completions")
    accepted = [attempts[i]["request_id"] for i in range(len(consumed))]
    active = None if len(worker_dispatched) == len(consumed) else attempts[len(worker_dispatched)-1]["attempt_id"]
    terminal, worker_generation = _session_lifetime(records, closure, identity, started, accepted, active, worker_image,
        unready_attempts_absent=not (started_ids or dispatched or worker_dispatched or completions or consumed))
    terminals = by_kind(aw_child, "session_completed")
    if len(terminals) != 1 or terminals[0] != aw_child[-1] or terminals[0].decoded()["payload"] != {"worker_terminal": closure["worker_terminal"]}:
        raise AccountingError("worker terminal lacks its exact final control record")
    _same(terminal["last_worker_message_sha256"], terminals[0].decoded()["previous_message_sha256"], "worker terminal predecessor")
    owner_head = (aw_owner[-1].binding["sha256"] if aw_owner else hashlib.sha256(control.canonical({
        "domain": "iroha:private-settlement:session-control-chain:v1", "session_started_sha256": closure["session_started"]["sha256"],
        "channel": "adapter_worker", "direction": "owner_to_child"})).hexdigest())
    _same(terminal["last_owner_message_sha256"], owner_head, "worker consumed owner frontier")
    if terminal["kind"] == "completed" and (len(accepted) != len(attempts) or any(row["state"] != "succeeded" for row in rows)):
        raise AccountingError("completed session omits an accepted attempt")
    stopped = False
    for row in rows:
        if stopped and row["state"] != "not_started":
            raise AccountingError("session dispatched after an unsuccessful attempt")
        if row["state"] != "succeeded":
            stopped = True
    return rows, samples, {"session_id": sid, "session_invocation_nonce": identity["session_invocation_nonce"],
        "session_request_sha256": identity["session_request_sha256"], "started_request_ids": started_ids,
        "attempt_nonces": sorted(nonces), "terminal_kind": terminal["kind"],
        "started_ns": started["started_ns"], "closed_ns": closure["closed_ns"],
        "worker_generation_sha256": worker_generation, "closure": packet["closure"], "counts": _counter(rows),
        "setup_outcome": "failed_before_readiness" if (closure["ready"] is None
            and terminal["kind"] == "failed" and terminal["reason"] == "setup_failed") else None,
        "network_cleanup_claimed": terminal["network_shutdown_observed"] is True
                                   and terminal["coordinator_reaped_observed"] is True}


def _retained_plan(plan):
    """Regenerate every first-release session, including all warmup cohorts."""
    exact_fields(plan, PLAN_FIELDS | {"benchmark_sessions", "workload_manifests"}, "retained benchmark plan")
    _header(plan, "retained benchmark plan")
    if plan["worktree_clean"] is not True or plan["publication_evidence"] is not False or plan["execution_required"] is not True:
        raise AccountingError("plan lacks its source/execution contract")
    descriptors = plan["benchmark_sessions"]
    if type(descriptors) is not list or not descriptors:
        raise AccountingError("retained benchmark sessions must be registered upfront")
    configuration = {}; workloads = {}
    for descriptor in descriptors:
        exact_fields(descriptor, frozenset({"session_id", "profile", "participants", "seed",
            "configuration_sha256", "workload_manifest_sha256", "warmup_attempts", "measured_attempts"}), "session descriptor")
        n = descriptor["participants"]
        configuration.setdefault(n, descriptor["configuration_sha256"])
        workloads.setdefault(n, descriptor["workload_manifest_sha256"])
    seeds = [row["seed"] for row in descriptors if row["profile"] == "private" and row["participants"] == 2]
    measured = sum(row["measured_attempts"] for row in descriptors if row["profile"] == "private" and row["participants"] == 2)
    expected = build_benchmark_session_plan(configuration, seeds, workloads,
        warmups_per_session=descriptors[0]["warmup_attempts"], measured_per_profile=measured)
    _same(descriptors, expected["sessions"], "complete registered retained session inventory")
    indexed = []
    all_ids = []
    for ordinal, job in enumerate(plan["jobs"], 1):
        if type(job) is not dict or job.get("kind") not in {"benchmark", "fault", "leakage"}:
            raise AccountingError("full plan has an unknown job")
        _same(job.get("request_id"), hashlib.sha256(accounting_canonical_bytes({
            key: value for key, value in job.items() if key != "request_id"})).hexdigest(), "planned request identity")
        all_ids.append(job["request_id"])
        if job["kind"] == "benchmark": indexed.append((ordinal, job))
    if len(set(all_ids)) != len(all_ids):
        raise AccountingError("full plan repeats a request")
    _same([job for _, job in indexed], expected["jobs"], "complete registered benchmark jobs")
    if [ordinal for ordinal, _ in indexed] != list(range(indexed[0][0], indexed[0][0] + len(indexed))):
        raise AccountingError("nonbenchmark job interrupts retained benchmark sessions")
    return indexed


def _retained_scope_inputs(scope_raw, worker_command, worker_image, validate_success):
    """Share exact registration/native admission across campaign and scope cuts."""
    scope = exact_fields(_document(scope_raw, "scope"), SCOPE_FIELDS, "scope")
    _header(scope, "scope")
    scope_binding = accounting_file_binding(scope_raw)
    _digest(scope["scope_id"], "scope id")
    if scope["previous_scope_sha256"] is not None or scope["stopping_policy"] != "fail_fast":
        raise AccountingError("scope must register its complete first-release fail-fast inventory")
    registered = unsigned_milliseconds(scope["registered_ns"], "registration", positive=True)
    policy = validate_deadline_policy(scope["deadline_policy"])
    _binding(worker_image, "source-admitted native worker")
    if type(worker_command) is not list or not worker_command or any(type(x) is not str or not x for x in worker_command):
        raise AccountingError("native worker command is missing")
    if not callable(validate_success):
        raise AccountingError("canonical retained semantic replay is mandatory")
    slots = scope["campaigns"]
    if type(slots) is not list or not slots:
        raise AccountingError("registered campaign inventory is incomplete")
    ids = []
    for slot in slots:
        exact_fields(slot, frozenset({"campaign_id", "plan"}), "campaign registration")
        ids.append(_campaign_id(slot["campaign_id"]))
        _binding(slot["plan"], "registered plan")
    if ids != sorted(set(ids)):
        raise AccountingError("campaign inventory omits or reorders a registered slot")
    return scope, scope_binding, registered, policy, ids


def validate_serial_owner_lifetimes(intervals, registered_ns):
    """Require exact full-plan process/session lifetimes to be serial."""
    previous = unsigned_milliseconds(registered_ns, "owner registration", positive=True)
    for started, finished in intervals:
        started = unsigned_milliseconds(started, "owner start", positive=True)
        finished = unsigned_milliseconds(finished, "owner finish", positive=True)
        if started < previous or finished < started:
            raise AccountingError("full-plan process/session owner lifetimes overlap or reorder")
        previous = finished


def _reduce_retained_campaign(slot, packet, *, scope_binding, registered, policy,
                              worker_command, worker_image, validate_success,
                              generations, nonces, workers):
    """Single owner of session, process, prefix and campaign closure predicates."""
    samples = []
    exact_fields(packet, frozenset({"campaign_id", "plan", "closure", "sessions", "records", "nonbenchmark"}), "retained campaign packet")
    _same(accounting_file_binding(packet["plan"]), slot["plan"], "registered plan bytes")
    plan = _document(packet["plan"], "plan")
    indexed = _retained_plan(plan)
    _same(validate_deadline_policy(plan["benchmark_accounting"]), policy, "registered deadline policy")
    current_environment = {key: plan[key] for key in ("commit", "hardware", "benchmark_accounting")}
    identity = {"scope_sha256": scope_binding["sha256"], "campaign_id": slot["campaign_id"], "plan_sha256": slot["plan"]["sha256"]}
    closure = exact_fields(_document(packet["closure"], "campaign closure"), CLOSURE_FIELDS | {
        "started_session_ids", "session_closures"}, "retained campaign closure")
    _record_identity(closure, identity, "campaign closure")
    closed = unsigned_milliseconds(closure["closed_ns"], "campaign cut", positive=True)
    if closure["quiescent"] is not True or closed < registered or closure["reason"] not in {
        "completed", "fail_fast", "preparation_failed", "recovered_interruption", "not_run"}:
        raise AccountingError("campaign has no authoritative quiescent cut")
    records = _SessionRecords(packet["records"])
    if type(packet["sessions"]) is not list or len(packet["sessions"]) != len(plan["benchmark_sessions"]):
        raise AccountingError("campaign omits a planned session")
    campaign_rows, session_summaries, session_refs, session_ids = [], [], [], []
    stopped = False
    preceding_closed = registered
    for descriptor, session in zip(plan["benchmark_sessions"], packet["sessions"]):
        if stopped and session is not None:
            raise AccountingError("campaign started a session after unsuccessful or unstarted predecessor")
        reduced, measured, summary = reduce_retained_session(descriptor, session, records=records,
            campaign_identity=identity, jobs=indexed, registered_ns=registered, closed_ns=closed,
            policy=policy, worker_command=worker_command, worker_image=worker_image,
            validate_success=validate_success)
        campaign_rows.extend(reduced); samples.extend(measured)
        if summary is None:
            stopped = True
        else:
            generation = summary["session_invocation_nonce"]
            if generation in generations or nonces.intersection(summary["attempt_nonces"]):
                raise AccountingError("scope reuses a session or attempt generation")
            generations.add(generation); nonces.update(summary.pop("attempt_nonces"))
            if summary["started_ns"] < preceding_closed:
                raise AccountingError("retained sessions overlap their exact owner lifetimes")
            preceding_closed = summary["closed_ns"]
            worker_generation = summary["worker_generation_sha256"]
            if worker_generation is not None:
                if worker_generation in workers:
                    raise AccountingError("distinct sessions reuse the same native worker generation")
                workers.add(worker_generation)
            session_summaries.append(summary); session_ids.append(descriptor["session_id"])
            session_refs.append(summary["closure"])
            stopped = summary["terminal_kind"] != "completed"
    _same(closure["started_session_ids"], session_ids, "campaign session starts")
    _same(closure["session_closures"], session_refs, "campaign session closures")
    actual_starts = {row["request_id"] for row in campaign_rows if row["state"] != "not_started"}
    benchmark_states = {row["request_id"]: row["state"] for row in campaign_rows}
    nonbenchmark_jobs = [(i, job) for i, job in enumerate(plan["jobs"], 1) if job["kind"] != "benchmark"]
    if type(packet["nonbenchmark"]) is not list or len(packet["nonbenchmark"]) != len(nonbenchmark_jobs):
        raise AccountingError("full-plan nonbenchmark closure inventory differs")
    states = dict(benchmark_states)
    owner_intervals = {next(i for i, job in enumerate(plan["jobs"], 1)
        if job.get("session_id") == summary["session_id"]):
        (summary["started_ns"], summary["closed_ns"]) for summary in session_summaries}
    for (ordinal, job), other in zip(nonbenchmark_jobs, packet["nonbenchmark"]):
        exact_fields(other, frozenset({"request_id", "started", "request", "process"}), "nonbenchmark packet")
        _same(other["request_id"], job["request_id"], "nonbenchmark request")
        if other["started"] is None:
            if other["process"] is not None: raise AccountingError("nonbenchmark process lacks a durable start")
            states[job["request_id"]] = "not_started"; continue
        start = exact_fields(_document(other["started"], "nonbenchmark start"), START_FIELDS, "nonbenchmark start")
        owner = {**identity, "attempt_id": registered_attempt_id(**identity, request_id=job["request_id"]),
                 "request_id": job["request_id"], "invocation_nonce": _digest(start["invocation_nonce"], "invocation nonce")}
        _record_identity(start, owner, "nonbenchmark start")
        if not registered <= start["started_ns"] <= closed or start["timeout_seconds"] * 1000 != policy["outer_timeout_ms"]:
            raise AccountingError("nonbenchmark start differs from registered cut or deadline")
        _same(start["request"], accounting_file_binding(other["request"]), "nonbenchmark request bytes")
        _same(start["harness"], plan["harness"], "nonbenchmark harness")
        if owner["invocation_nonce"] in nonces: raise AccountingError("full plan repeats an invocation nonce")
        nonces.add(owner["invocation_nonce"])
        process = _process_record(_document(other["process"], "nonbenchmark process"), owner, start, closed, policy)
        if process["owned_process_group_gone"] is not True:
            raise AccountingError("nonbenchmark process is not quiescent")
        states[job["request_id"]] = ("succeeded" if process["completion_kind"] == "exited"
            and process["exit_code"] == 0 and process["passed"] is True else "incomplete")
        actual_starts.add(job["request_id"])
        owner_intervals[ordinal] = (start["started_ns"], process["finished_ns"])
    validate_serial_owner_lifetimes([owner_intervals[key] for key in sorted(owner_intervals)], registered)
    expected_starts = [job["request_id"] for job in plan["jobs"] if job["request_id"] in actual_starts]
    _same(closure["started_request_ids"], expected_starts, "full-plan durable start inventory")
    _same(expected_starts, [job["request_id"] for job in plan["jobs"][:len(expected_starts)]], "full-plan dispatch prefix")
    for summary in session_summaries:
        first = next(i for i, job in enumerate(plan["jobs"]) if job.get("session_id") == summary["session_id"])
        if any(states[job["request_id"]] != "succeeded" for job in plan["jobs"][:first]):
            raise AccountingError("session owner started after an unsuccessful full-plan predecessor")
    stopped = False
    for job in plan["jobs"]:
        state = states[job["request_id"]]
        if stopped and state != "not_started":
            raise AccountingError("full campaign continued after its first unsuccessful attempt")
        stopped |= state != "succeeded"
    if closure["reason"] == "completed" and (stopped or any(s["terminal_kind"] != "completed" for s in session_summaries)):
        raise AccountingError("completed campaign has an unfinished attempt or session")
    if closure["reason"] == "not_run" and (actual_starts or session_ids):
        raise AccountingError("unstarted campaign contains a session owner")
    summary = {"campaign_id": slot["campaign_id"], "plan_sha256": slot["plan"]["sha256"],
                      "counts": _counter(campaign_rows), "sessions": session_summaries}
    records.validate()
    return campaign_rows, samples, summary, current_environment


def _retained_sample_inventory(successful_rows, samples):
    """Join the complete retained sample inventory to independent replay."""
    observed = {}
    for raw in successful_rows:
        value = _document(raw, "published successful sample")
        key = _digest(value.get("attempt_id"), "sample attempt id")
        if key in observed: raise AccountingError("duplicate published successful sample")
        observed[key] = raw
    expected = {_document(raw, "retained sample")["attempt_id"]: raw for raw in samples}
    if len(expected) != len(samples) or observed != expected:
        raise AccountingError("successful sample inventory omits, changes or adds an attempt")


def _retained_cohorts(rows):
    """Count every registered cohort including untouched closed tails."""
    cohorts = []
    for profile, n, warmup in sorted({(r["profile"], r["participants"], r["warmup"]) for r in rows}):
        cohorts.append({"profile": profile, "participants": n, "warmup": warmup,
            "counts": _counter([r for r in rows if (r["profile"], r["participants"], r["warmup"]) == (profile, n, warmup)])})
    return cohorts


def reduce_retained_campaign(scope_raw, packet, successful_rows, *, worker_command,
                             worker_image, validate_success):
    """Validate one actual campaign cut without inventing other scope closures."""
    scope, binding, registered, policy, ids = _retained_scope_inputs(
        scope_raw, worker_command, worker_image, validate_success)
    if packet.get('campaign_id') not in ids:
        raise AccountingError('campaign is absent from the complete registered scope')
    slot = scope['campaigns'][ids.index(packet['campaign_id'])]
    rows, samples, summary, _ = _reduce_retained_campaign(slot, packet, scope_binding=binding,
        registered=registered, policy=policy, worker_command=worker_command, worker_image=worker_image,
        validate_success=validate_success, generations=set(), nonces=set(), workers=set())
    _retained_sample_inventory(successful_rows, samples)
    return {'campaign': summary, 'counts': _counter(rows), 'rows': rows,
            'accounting_complete': all(row['state'] != 'incomplete' for row in rows),
            'registered_scope_complete': False}


def reduce_registered_scope(scope_raw, campaigns, successful_rows, *, worker_command,
                            worker_image, validate_success):
    """Reduce closed retained sessions; one-shot benchmark packets are rejected.

    File inventories, native image admission and semantic replay are mandatory
    caller-owned inputs. Fault/leakage jobs retain their existing process record
    contract and never enter benchmark counts. No reads, execution or metrics
    fabrication occur here; unclosed sessions cannot establish missing tails.
    """
    scope, scope_binding, registered, policy, ids = _retained_scope_inputs(
        scope_raw, worker_command, worker_image, validate_success)
    slots = scope['campaigns']
    if (type(campaigns) not in (list, tuple) or len(campaigns) != len(slots)
            or [packet.get('campaign_id') for packet in campaigns] != ids):
        raise AccountingError('registered campaign inventory is incomplete or reordered')
    rows, samples, summaries, generations, nonces, workers = [], [], [], set(), set(), set()
    environment = None
    for slot, packet in zip(slots, campaigns):
        campaign_rows, measured, summary, current_environment = _reduce_retained_campaign(slot, packet,
            scope_binding=scope_binding, registered=registered, policy=policy, worker_command=worker_command,
            worker_image=worker_image, validate_success=validate_success,
            generations=generations, nonces=nonces, workers=workers)
        if environment is not None:
            _same(current_environment, environment, 'scope campaign environment')
        environment = current_environment
        rows.extend(campaign_rows); samples.extend(measured); summaries.append(summary)
    _retained_sample_inventory(successful_rows, samples)
    cohorts = _retained_cohorts(rows)
    return {"version": VERSION, "protocol": PROTOCOL, "scope_id": scope["scope_id"], "scope": scope_binding,
        "previous_scope_sha256": None, "timeout_scope": TIMEOUT_SCOPE, "deadline_policy": policy,
        "counts": _counter(rows), "cohorts": cohorts, "campaigns": summaries, "rows": rows,
        "accounting_complete": all(row["state"] != "incomplete" for row in rows)}


if __name__ == "__main__":
    argparse.ArgumentParser(description=__doc__).parse_args()
