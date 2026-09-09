#!/usr/bin/env python3
"""Validate a Sumeragi V2 one-lane/four-lane horizontal-scaling evidence bundle.

The validator uses only the Python standard library.  It treats the bundle as
untrusted release evidence: JSON must be strict, referenced files must be
regular in-bundle files with matching SHA-256 digests, all ten runs must be
present in canonical pair order, and every recorded value is recomputed before
the scaling thresholds are evaluated.
"""

from __future__ import annotations

import argparse
import errno
import hashlib
import json
import math
import os
import re
import secrets
import stat
import statistics
import sys
from dataclasses import dataclass, replace
from datetime import datetime, timezone
from fractions import Fraction
from pathlib import Path, PurePosixPath
from typing import Any, NoReturn, Sequence

from resource_bundle import ControlBinding
from resource_evidence_budget import EvidenceBudget, select_run_budget
from resource_experiment import ResourceExperiment, RunReplayInput


EVIDENCE_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.evidence.v1"
RUN_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.run.v1"
IDENTITY_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.identity.v1"
REPORT_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.validation.v1"
TRACE_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.trace.v1"
EXPECTED_PAIR_COUNT = 5
MIN_INTERVAL_SAMPLES = 20
MIN_LATENCY_SAMPLES = 100
MIN_THROUGHPUT_RATIO = 1.5
MAX_P95_LATENCY_RATIO = 1.25
MAX_OFFERED_LOAD_DEVIATION_FRACTION = 0.01
SEED_DERIVATION = "sha256(seed_namespace + ':' + decimal_pair_index)"
MAX_BUNDLE_FILE_COUNT = 256
MAX_BUNDLE_FILE_BYTES = 256 * 1024 * 1024
MAX_BUNDLE_TOTAL_BYTES = 2 * 1024 * 1024 * 1024
MAX_TRANSACTION_TRACE_ROWS = 1_000_000
MAX_DRAIN_SECONDS = 300
NANOSECONDS_PER_SECOND = 1_000_000_000
LOGICAL_ID_DERIVATION = "sha256(seed + ':' + cohort + ':' + decimal_sequence)"

REQUIRED_TOOLING = (
    ("localnet", "scripts/deploy_localnet.sh"),
    ("load_generator", "scripts/tx_load.py"),
    ("nexus_load_bundle", "scripts/nexus_lane_load_test.py"),
)

_DIGEST_RE = re.compile(r"^[0-9a-f]{64}$")
# The SDK's is_canonical_signed_transaction_hash_text and Hash::prehashed
# require lowercase hex with the final least-significant bit set.
_TRANSACTION_HASH_RE = re.compile(r"^[0-9a-f]{63}[13579bdf]$")
_REVISION_RE = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
_SEED_NAMESPACE_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
_SAFE_PATH_COMPONENT_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")

_MANIFEST_FIELDS = {
    "schema",
    "generated_at_utc",
    "pair_count",
    "seed_namespace",
    "seed_derivation",
    "identity",
    "configuration",
    "workload",
    "budgets",
    "observation_scope",
    "thresholds",
    "trial_harness",
    "validator",
    "tooling",
    "runs",
}
_IDENTITY_FIELDS = {"schema", "hardware", "software"}
_HARDWARE_FIELDS = {
    "machine_id",
    "cpu_model",
    "physical_core_count",
    "logical_core_count",
    "memory_bytes",
    "storage_model",
}
_SOFTWARE_FIELDS = {
    "os",
    "kernel",
    "architecture",
    "python_version",
    "rustc_version",
    "source_revision",
    "workspace_source_sha256",
    "nexus_config_sha256",
    "irohad_sha256",
    "iroha_cli_sha256",
}
_WORKLOAD_FIELDS = {
    "offered_load_tps",
    "warmup_seconds",
    "measurement_seconds",
    "drain_seconds",
    "max_submission_lag_ms",
    "min_interval_samples",
    "min_latency_samples",
    "max_offered_load_deviation_fraction",
}
_BUDGET_FIELDS = {
    "queue_depth_max",
    "index_entries_max",
    "memory_bytes_max",
    "disk_bytes_max",
}
_SCOPE_FIELDS = {"queue", "index", "memory", "disk"}
_THRESHOLD_FIELDS = {
    "min_four_lane_throughput_ratio",
    "max_four_lane_p95_latency_ratio",
}
_TOOL_FIELDS = {"role", "source_path", "artifact"}
_RUN_ENTRY_FIELDS = {
    "sequence",
    "pair_index",
    "variant",
    "active_execution_lanes",
    "seed",
    "status",
    "skipped",
    "exit_code",
    "raw_samples",
    "command_log",
}
_RAW_RUN_FIELDS = {
    "schema",
    "pair_index",
    "variant",
    "active_execution_lanes",
    "execution_lane_ids",
    "seed",
    "identity_before",
    "identity_after",
    "workload",
    "status",
    "summary",
    "samples",
    "warmup",
    "drain",
    "artifacts",
}
_STATUS_FIELDS = {"outcome", "skipped", "failure"}
_SUMMARY_FIELDS = {
    "offered_count",
    "accepted_count",
    "committed_count",
    "queue_depth_max",
    "index_entries_max",
    "memory_bytes_max",
    "disk_bytes_max",
}
_SAMPLE_FIELDS = {
    "sequence",
    "start_offset_seconds",
    "end_offset_seconds",
    "offered_count",
    "accepted_count",
    "committed_count",
    "commit_latencies_ms",
    "queue_depth",
    "index_entries",
    "memory_bytes",
    "disk_bytes",
}
_FILE_REF_FIELDS = {"path", "sha256"}
_NEXUS_LOAD_INPUT_FIELDS = {
    "lifecycle_file",
    "metrics_file",
    "telemetry_file",
    "alias_migrations",
}
_RUN_ARTIFACT_FIELDS = {
    "nexus_load_test_manifest",
    "lifecycle_snapshot",
    "metrics_snapshot",
    "load_generator_log",
    "transaction_trace",
    "collector_journal",
    "canonical_proof",
}
_COUNT_FIELDS = {"offered_count", "accepted_count", "committed_count"}
_TRACE_FIELDS = {
    "schema", "pair_index", "variant", "seed", "clock",
    "logical_id_derivation", "transaction_hash_source", "transactions",
}
_TRANSACTION_FIELDS = {
    "cohort", "sequence", "logical_id", "hash", "scheduled_offset_ns",
    "offer_offset_ns", "submission_lag_ns", "acknowledgment", "applied",
}
_ACK_FIELDS = {"offset_ns", "hash", "status", "rejection"}
_APPLIED_FIELDS = {
    "offset_ns", "hash", "scope", "resolved_from", "status", "block_height",
}


class EvidenceError(ValueError):
    """The supplied release-evidence bundle violates the G-SCALE contract."""


@dataclass(frozen=True)
class RunMetrics:
    """Metrics recomputed from one raw run."""

    pair_index: int
    variant: str
    lane_ids: tuple[str, ...]
    offered_count: int
    accepted_count: int
    committed_count: int
    throughput_tps: float
    p95_latency_ms: float
    interval_sample_count: int
    latency_sample_count: int
    latencies_ms: tuple[float, ...]
    maxima: dict[str, int]
    cohort_accepted_count: int = 0
    drain_accepted_count: int = 0
    drain_committed_count: int = 0
    drain_rejected_count: int = 0


def _fail(message: str) -> NoReturn:
    raise EvidenceError(message)


def _strict_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail(f"duplicate JSON object key: {key}")
        result[key] = value
    return result


def _reject_constant(value: str) -> NoReturn:
    _fail(f"nonfinite JSON numeric literal is forbidden: {value}")


@dataclass(frozen=True, slots=True)
class ResourceAdmission:
    """Independent launcher authority; no member is learned from a capture."""

    budget: EvidenceBudget
    controls: tuple[ControlBinding, ...]
    runs: tuple[RunReplayInput, ...]
    expected_executable_sha256: str
    reported: bool


_STATIC_ROLES = ('identity', 'configuration', 'trial_harness', 'validator',
                 'localnet', 'load_generator', 'nexus_load_bundle')
_SUPPORT_ROLES = ('nexus_load_test_manifest', 'lifecycle_snapshot',
                  'metrics_snapshot', 'load_generator_log')
MAX_METADATA_JSON_BYTES = 4 * 1024 * 1024
MAX_RAW_RUN_JSON_BYTES = 32 * 1024 * 1024
MAX_TRACE_JSON_BYTES = 256 * 1024 * 1024


class EvidenceControls:
    """Resolve semantic roles to original admitted bindings without path opens.

    The G-SCALE schema fixes static labels and each run's four support labels.
    The five principal run allocations carry their own explicit role labels.
    References must match both the original path and independently pinned hash.
    """

    def __init__(self, owner: ResourceExperiment, admission: ResourceAdmission):
        if type(owner) is not ResourceExperiment or type(admission) is not ResourceAdmission:
            _fail('exact retained resource admission required')
        self.owner = owner
        self.admission = admission
        self.budget = select_run_budget(admission.budget, 1, 'one_lane').experiment
        if {item.label for item in self.budget.static_files} != set(_STATIC_ROLES):
            _fail('G-SCALE requires exactly the seven independently pinned static roles')
        if len(self.budget.control_budgets) != 2:
            _fail('G-SCALE control allocations must be exactly manifest and report')
        self.bindings = {item.label: item for item in admission.controls}
        self.caps = {item.label: item.max_bytes for item in
                     (*self.budget.control_budgets,
                      *(item for run in self.budget.runs for item in run.files))}
        self.caps.update({item.label: item.size_bytes for item in self.budget.static_files})
        for run in self.budget.runs:
            expected = {f'pair-{run.pair_index:02}.{run.variant}.{role}' for role in _SUPPORT_ROLES}
            if {item.label for item in run.support} != expected:
                _fail('G-SCALE run requires exactly its four canonical support roles')

    def binding(self, label: str) -> ControlBinding:
        """Return the exact original role binding; no inferred path is accepted."""
        try:
            return self.bindings[label]
        except KeyError:
            _fail('independently admitted control role is missing')

    def read(self, binding: ControlBinding, *, max_bytes: int) -> bytes:
        """Enforce the semantic cap and the original role's smaller allocation."""
        if type(max_bytes) is not int or not 0 <= max_bytes <= MAX_BUNDLE_FILE_BYTES:
            _fail('invalid semantic control byte cap')
        if type(binding) is not ControlBinding or self.bindings.get(binding.label) is not binding:
            _fail('control is not an original admitted role binding')
        return self.owner.read_control(binding, max_bytes=min(max_bytes, self.caps[binding.label]))


def load_json(binding: ControlBinding, label: str, *, controls: EvidenceControls,
              max_bytes: int) -> Any:
    """Decode strict bounded UTF-8 JSON from the retained semantic reader."""
    raw = controls.read(binding, max_bytes=max_bytes)
    return _decode_json_bytes(raw, label, max_bytes=max_bytes)


def _decode_json_bytes(raw: bytes, label: str, *, max_bytes: int) -> Any:
    """Bound a single JSON decode before depth, numeric and UTF-8 validation."""
    if (type(raw) is not bytes or type(max_bytes) is not int
            or not 0 < len(raw) <= max_bytes <= MAX_BUNDLE_FILE_BYTES):
        _fail(f'{label} exceeds its JSON byte limit')
    depth, quoted, escaped = 0, False, False
    for byte in raw:
        if quoted:
            if escaped: escaped = False
            elif byte == 92: escaped = True
            elif byte == 34: quoted = False
        elif byte == 34: quoted = True
        elif byte in (91, 123):
            depth += 1
            if depth > 64: _fail(f'{label} exceeds JSON depth limit')
        elif byte in (93, 125):
            depth -= 1
            if depth < 0: _fail(f'{label} is not strict UTF-8 JSON')
    if depth != 0 or quoted:
        _fail(f'{label} is not strict UTF-8 JSON')
    def integer(token):
        if len(token) > 128: _fail(f'{label} exceeds JSON numeric token limit')
        return int(token)
    def real(token):
        if len(token) > 128: _fail(f'{label} exceeds JSON numeric token limit')
        value = float(token)
        if not math.isfinite(value): _fail(f'{label} contains a nonfinite JSON number')
        return value
    try:
        return json.loads(raw.decode('utf-8'), object_pairs_hook=_strict_object,
                          parse_int=integer, parse_float=real, parse_constant=_reject_constant)
    except EvidenceError:
        raise
    except (ValueError, UnicodeError, RecursionError):
        _fail(f'{label} is not strict UTF-8 JSON')


def _reconcile_journal_trace(journal: bytes, rows: list[dict[str, Any]], *, pair_index: int,
                             variant: str, seed: str, geometry, submission_lag_bound_ns: int) -> tuple[str, ...]:
    """Bind the validated trace to the collector's ordered durable workload rows.

    Resource replay separately authenticates all capture references and Clock
    events. This pass retains only the fixed account pool and its at-most-64
    observed postcondition digests, without collecting journal lines in memory.
    It establishes agreement with the collector log, not canonical state proof.
    """
    if (type(journal) is not bytes or not 0 < len(journal) <= MAX_BUNDLE_FILE_BYTES
            or not journal.endswith(b'\n') or type(rows) is not list
            or not 0 < len(rows) <= MAX_TRANSACTION_TRACE_ROWS):
        _fail('journal/trace bounded framing is invalid')
    if (type(submission_lag_bound_ns) is not int
            or not 0 <= submission_lag_bound_ns < 1 << 63):
        _fail('independent submission lag bound is invalid')
    accounts: tuple[str, ...] = ()
    preflight_digests: list[str] = []
    scheduled = final = postconditions = 0
    clock_started = resource_finished = post_started = finished = False
    offset = line_index = account_offset = effects = 0

    def equal(actual, expected):
        if type(actual) is not type(expected):
            _fail('collector journal disagrees with the exact trace or workload')
        if type(expected) is dict:
            if actual.keys() != expected.keys():
                _fail('collector journal disagrees with the exact trace or workload')
            for key, value in expected.items():
                equal(actual[key], value)
        elif type(expected) is list:
            if len(actual) != len(expected):
                _fail('collector journal disagrees with the exact trace or workload')
            for left, right in zip(actual, expected, strict=True):
                equal(left, right)
        elif actual != expected:
            _fail('collector journal disagrees with the exact trace or workload')

    def plan_for(row):
        return {name: row[name] for name in ('cohort', 'sequence', 'logical_id', 'scheduled_offset_ns')} | {
            'account_index': (row['sequence'] - 1 + account_offset) % len(accounts)}

    while offset < len(journal):
        end = journal.find(b'\n', offset)
        if end < 0 or not 0 < end - offset <= 16 * 1024 or finished:
            _fail('journal event framing or terminal order is invalid')
        row = _require_object(_decode_json_bytes(journal[offset:end], 'collector event',
                                                max_bytes=16 * 1024), 'collector event')
        offset = end + 1
        event = row.get('event')
        if line_index == 0:
            equal(event, 'plan')
            equal(row.get('pair_index'), pair_index)
            equal(row.get('variant'), variant)
            equal(row.get('seed'), seed)
            equal(row.get('scheduled_requests'), len(rows))
            equal(row.get('workload'), 'self_owned_account_metadata_insert_v1')
            equal(row.get('account_selection'), '(zero_based_cohort_sequence + first8le(sha256(gscale-account-offset-v1:seed))) modulo pool_length')
            equal(row.get('max_effects_per_account'), 1024)
            equal(row.get('submission_lag_bound_ns'), submission_lag_bound_ns)
            for name in ('warmup_ns', 'measurement_ns', 'drain_ns', 'preparation_ahead_ns'):
                equal(row.get(name), getattr(geometry, name))
            pool = _require_list(row.get('accounts'), 'collector account pool')
            if not 4 <= len(pool) <= 64 or len(pool) % 4:
                _fail('collector account pool must contain complete four-account groups')
            authorities = []
            for item in pool:
                item = _require_object(item, 'collector account')
                _require_exact_fields(item, {'authority'}, 'collector account')
                authority = _require_text(item['authority'], 'collector authority')
                if len(authority) > 2048: _fail('collector authority exceeds its bound')
                authorities.append(authority)
            accounts = tuple(authorities)
            if len(set(accounts)) != len(accounts): _fail('collector account pool duplicates authority')
            warmup = sum(item['cohort'] == 'warmup' for item in rows)
            if warmup % len(accounts) or (len(rows) - warmup) % len(accounts):
                _fail('collector cohorts do not contain complete account-pool rounds')
            effects = len(rows) // len(accounts)
            if not 1 <= effects <= 1024: _fail('collector useful-effect count exceeds its bound')
            account_offset = int.from_bytes(hashlib.sha256(
                f'gscale-account-offset-v1:{seed}'.encode('ascii')).digest()[:8], 'little') % len(accounts)
        elif event == 'plan':
            _fail('collector has more than one workload plan')
        elif event == 'scheduled':
            if clock_started or scheduled >= len(rows): _fail('scheduled journal rows are missing or reordered')
            _require_exact_fields(row, {'event', 'index', 'plan'}, 'scheduled journal row')
            equal(row['index'], scheduled)
            equal(row['plan'], plan_for(rows[scheduled]))
            scheduled += 1
        elif event == 'workload_account_preflight':
            index = len(preflight_digests)
            if clock_started or scheduled != len(rows) or index >= len(accounts):
                _fail('workload preflight is incomplete or outside its clock boundary')
            _require_exact_fields(row, {'event', 'authority', 'account_index', 'expected_effects',
                'expected_account_sha256', 'expected_account_frame_bytes'}, 'workload preflight')
            equal(row['authority'], accounts[index])
            equal(row['account_index'], index)
            equal(row['expected_effects'], effects)
            size = _require_int(row['expected_account_frame_bytes'], 'account frame bytes', minimum=1)
            if size > 256 * 1024: _fail('workload account frame exceeds its bound')
            preflight_digests.append(_require_digest(row['expected_account_sha256'], 'expected account digest'))
        elif event == 'clock_started':
            if clock_started or scheduled != len(rows) or len(preflight_digests) != len(accounts):
                _fail('Clock started before complete scheduled workload preflight')
            clock_started = True
        elif event == 'resource_collection_finished':
            if not clock_started or resource_finished: _fail('resource finish journal order is invalid')
            resource_finished = True
        elif event == 'workload_postconditions_started':
            if not resource_finished or post_started: _fail('workload postcondition start order is invalid')
            post_started = True
        elif event == 'workload_account_postcondition':
            if not post_started or postconditions >= len(accounts):
                _fail('workload postcondition cohort is incomplete or reordered')
            _require_exact_fields(row, {'event', 'authority', 'account_index', 'verified_effects',
                'account_sha256', 'read_source'}, 'workload postcondition')
            equal(row['authority'], accounts[postconditions])
            equal(row['account_index'], postconditions)
            equal(row['verified_effects'], effects)
            equal(row['account_sha256'], preflight_digests[postconditions])
            equal(row['read_source'], 'signed_find_account_by_id_after_complete_drain')
            postconditions += 1
        elif event == 'request_final':
            if postconditions != len(accounts) or final >= len(rows):
                _fail('final workload rows are incomplete or reordered')
            expected = rows[final]
            equal(expected['acknowledgment']['status'], 'Accepted')
            if type(expected['applied']) is not dict: _fail('complete collector lacks StateApplied')
            equal(row.get('plan'), plan_for(expected))
            scheduled_ns = _offset_ns(expected['scheduled_offset_ns'], 'scheduled offset')
            offered_ns = _offset_ns(expected['offer_offset_ns'], 'offer offset')
            actual_lag = offered_ns - scheduled_ns
            equal(expected['submission_lag_ns'], actual_lag)
            if not 0 <= actual_lag <= submission_lag_bound_ns:
                _fail('collector offer exceeds the independently declared submission lag')
            for name, value in (
                ('hash', expected['hash']), ('offer_offset_ns', expected['offer_offset_ns']),
                ('acknowledgment_offset_ns', expected['acknowledgment']['offset_ns']),
                ('applied_offset_ns', expected['applied']['offset_ns']),
                ('block_height', expected['applied']['block_height']),
                ('submission_finished', True), ('failure', None),
            ):
                equal(row.get(name), value)
            _require_int(row.get('status_attempts'), 'status attempts')
            final += 1
        elif event == 'collection_finished':
            if final != len(rows): _fail('collector terminal row omits scheduled transactions')
            equal(row.get('passed'), True)
            equal(row.get('failure'), None)
            finished = True
        line_index += 1
    if not finished: _fail('collector journal is missing its complete terminal row')
    return accounts



def derive_seed(namespace: str, pair_index: int) -> str:
    """Derive the deterministic seed shared by both variants of one pair."""

    return hashlib.sha256(f"{namespace}:{pair_index}".encode("utf-8")).hexdigest()


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    return value


def _require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        _fail(f"{label} must be a list")
    return value


def _require_exact_fields(value: dict[str, Any], fields: set[str], label: str) -> None:
    actual = set(value)
    if actual != fields:
        missing = sorted(fields - actual)
        extra = sorted(actual - fields)
        _fail(f"{label} fields differ from schema; missing={missing}, extra={extra}")


def _require_text(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or "\x00" in value
        or "\n" in value
        or "\r" in value
    ):
        _fail(f"{label} must be a non-empty, trimmed single-line string")
    return value


def _require_safe_relative_path(value: Any, label: str) -> str:
    relative = _require_text(value, label)
    pure = PurePosixPath(relative)
    if (
        relative != pure.as_posix()
        or pure.is_absolute()
        or not pure.parts
        or any(
            part in {"", ".", ".."}
            or _SAFE_PATH_COMPONENT_RE.fullmatch(part) is None
            for part in pure.parts
        )
    ):
        _fail(f"{label} must be a normalized relative safe in-bundle path")
    return relative



def _require_int(value: Any, label: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum:
        _fail(f"{label} must be an integer >= {minimum}")
    return value


def _require_number(
    value: Any,
    label: str,
    *,
    minimum: float | None = None,
    strictly_positive: bool = False,
) -> float:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        _fail(f"{label} must be a finite number")
    try:
        number = float(value)
    except OverflowError:
        _fail(f"{label} must be a finite number")
    if not math.isfinite(number):
        _fail(f"{label} must be a finite number")
    if strictly_positive and number <= 0:
        _fail(f"{label} must be greater than zero")
    if minimum is not None and number < minimum:
        _fail(f"{label} must be >= {minimum}")
    return number


def _require_digest(value: Any, label: str) -> str:
    digest = _require_text(value, label)
    if _DIGEST_RE.fullmatch(digest) is None:
        _fail(f"{label} must be a lowercase 64-hex SHA-256 digest")
    return digest


def _require_ref(value: Any, controls: EvidenceControls, label: str, *,
                 expected_label: str, referenced_paths: set[str] | None = None) -> ControlBinding:
    ref = _require_object(value, label)
    _require_exact_fields(ref, _FILE_REF_FIELDS, label)
    relative = _require_safe_relative_path(ref['path'], f'{label}.path')
    expected_digest = _require_digest(ref['sha256'], f'{label}.sha256')
    binding = controls.binding(expected_label)
    if relative != binding.path or expected_digest != binding.sha256:
        _fail(f'{label} differs from its independently admitted role, path or digest')
    if referenced_paths is not None:
        referenced_paths.add(binding.path)
    return binding



def _require_timestamp(value: Any, label: str) -> None:
    raw = _require_text(value, label)
    if not raw.endswith("Z"):
        _fail(f"{label} must be an RFC3339 UTC timestamp ending in Z")
    try:
        parsed = datetime.fromisoformat(raw[:-1] + "+00:00")
    except ValueError as error:
        raise EvidenceError(f"{label} is not a valid RFC3339 timestamp") from error
    if parsed.tzinfo != timezone.utc:
        _fail(f"{label} must identify UTC")


def validate_identity(value: Any, label: str = "identity") -> dict[str, Any]:
    """Validate and return a pinned hardware/software identity object."""

    identity = _require_object(value, label)
    _require_exact_fields(identity, _IDENTITY_FIELDS, label)
    if identity["schema"] != IDENTITY_SCHEMA:
        _fail(f"{label}.schema must be {IDENTITY_SCHEMA!r}")

    hardware = _require_object(identity["hardware"], f"{label}.hardware")
    _require_exact_fields(hardware, _HARDWARE_FIELDS, f"{label}.hardware")
    for field in ("machine_id", "cpu_model", "storage_model"):
        _require_text(hardware[field], f"{label}.hardware.{field}")
    physical = _require_int(
        hardware["physical_core_count"],
        f"{label}.hardware.physical_core_count",
        minimum=1,
    )
    logical = _require_int(
        hardware["logical_core_count"],
        f"{label}.hardware.logical_core_count",
        minimum=1,
    )
    if physical > logical:
        _fail(f"{label}.hardware physical cores cannot exceed logical cores")
    _require_int(hardware["memory_bytes"], f"{label}.hardware.memory_bytes", minimum=1)

    software = _require_object(identity["software"], f"{label}.software")
    _require_exact_fields(software, _SOFTWARE_FIELDS, f"{label}.software")
    for field in (
        "os",
        "kernel",
        "architecture",
        "python_version",
        "rustc_version",
    ):
        _require_text(software[field], f"{label}.software.{field}")
    revision = _require_text(software["source_revision"], f"{label}.software.source_revision")
    if _REVISION_RE.fullmatch(revision) is None:
        _fail(f"{label}.software.source_revision must be lowercase 40- or 64-hex")
    for field in (
        "workspace_source_sha256",
        "nexus_config_sha256",
        "irohad_sha256",
        "iroha_cli_sha256",
    ):
        _require_digest(software[field], f"{label}.software.{field}")
    return identity


def _nearest_rank_p95(samples: Sequence[float]) -> float:
    if not samples:
        _fail("cannot compute p95 from an empty latency sample")
    ordered = sorted(samples)
    return ordered[math.ceil(0.95 * len(ordered)) - 1]


def _same_number(actual: float, expected: float) -> bool:
    return math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12)


def _nanoseconds(value: Any, label: str, *, scale: int = NANOSECONDS_PER_SECOND) -> int:
    """Require exact integer nanoseconds, without rounding a declared boundary."""

    number = _require_number(value, label, minimum=0)
    exact = Fraction(str(number)) * scale
    if exact.denominator != 1 or exact > (1 << 63) - 1:
        _fail(f"{label} must represent exact bounded integer nanoseconds")
    return int(exact)


def _offset_ns(value: Any, label: str) -> int:
    """Read a signed, bounded monotonic-clock offset; booleans are invalid."""

    if type(value) is not int or not -(1 << 63) < value < (1 << 63):
        _fail(f"{label} must be a bounded integer nanosecond offset")
    return value


def _schedule(workload: dict[str, Any]) -> tuple[Fraction, int, int, int, int]:
    """Check a fixed open-loop schedule before allocating or reading trace rows."""

    rate = Fraction(str(workload["offered_load_tps"]))
    warmup = _nanoseconds(workload["warmup_seconds"], "workload.warmup_seconds")
    measurement = _nanoseconds(workload["measurement_seconds"], "workload.measurement_seconds")
    drain = _nanoseconds(workload["drain_seconds"], "workload.drain_seconds")
    lag = _nanoseconds(workload["max_submission_lag_ms"], "workload.max_submission_lag_ms", scale=1_000_000)
    if drain == 0 or drain > MAX_DRAIN_SECONDS * NANOSECONDS_PER_SECOND:
        _fail(f"workload.drain_seconds must be positive and at most {MAX_DRAIN_SECONDS}")
    if warmup + measurement + 2 * drain >= 1 << 63:
        _fail("workload phase boundaries exceed bounded nanosecond offsets")
    period = Fraction(NANOSECONDS_PER_SECOND, 1) / rate
    if period < 1 or lag > period / 4:
        _fail("workload.max_submission_lag_ms must not exceed one quarter of an arrival period")
    row_count = math.ceil(rate * warmup / NANOSECONDS_PER_SECOND) + math.ceil(
        rate * measurement / NANOSECONDS_PER_SECOND
    )
    if row_count > MAX_TRANSACTION_TRACE_ROWS:
        _fail(f"workload schedule exceeds the {MAX_TRANSACTION_TRACE_ROWS} transaction trace row bound")
    return period, warmup, measurement, drain, lag


def _reconcile_event_samples(
    samples: list[Any],
    events: dict[str, list[tuple[int, int, float]]],
    *,
    label: str,
    final_inclusive: bool,
) -> None:
    """Bind interval counters and latency arrays to independent observed events."""

    ordered = {name: sorted(values) for name, values in events.items()}
    positions = {name: 0 for name in events}
    for index, sample in enumerate(samples):
        start = _nanoseconds(sample["start_offset_seconds"], f"{label}[{index}].start_offset_seconds")
        end = _nanoseconds(sample["end_offset_seconds"], f"{label}[{index}].end_offset_seconds")
        for name, values in ordered.items():
            first = positions[name]
            position = first
            while position < len(values) and (
                values[position][0] < end
                or (final_inclusive and index == len(samples) - 1 and values[position][0] == end)
            ):
                if values[position][0] < start:
                    _fail(f"{label}[{index}] leaves a transaction event outside its interval")
                position += 1
            if sample[name] != position - first:
                _fail(f"{label}[{index}].{name} disagrees with transaction trace events")
            if name == "committed_count":
                expected = [value[2] for value in values[first:position]]
                recorded = sample["commit_latencies_ms"]
                if len(recorded) != len(expected) or any(
                    not _same_number(actual, wanted)
                    for actual, wanted in zip(recorded, expected)
                ):
                    _fail(f"{label}[{index}].commit_latencies_ms disagrees with complete transaction trace latencies")
            positions[name] = position
    if any(positions[name] != len(values) for name, values in ordered.items()):
        _fail(f"{label} omits transaction trace events")


def _validate_run_trace(
    raw: dict[str, Any],
    metrics: RunMetrics,
    *,
    controls: EvidenceControls,
    workload: dict[str, Any],
    budgets: dict[str, Any],
    seen_transaction_hashes: set[str],
) -> RunMetrics:
    """Require a complete scheduled cohort, exact state completion, and its drain."""

    label = f"pair {metrics.pair_index} {metrics.variant}"
    period, warmup_ns, measurement_ns, drain_ns, lag_bound = _schedule(workload)
    trace = _require_object(load_json(
        _require_ref(raw["artifacts"]["transaction_trace"], controls,
                     f"{label} transaction trace", expected_label=select_run_budget(
                         controls.budget, metrics.pair_index, metrics.variant).run.transaction_trace.label),
        f"{label} transaction trace", controls=controls, max_bytes=MAX_TRACE_JSON_BYTES,
    ), f"{label} transaction trace")
    _require_exact_fields(trace, _TRACE_FIELDS, f"{label} transaction trace")
    expected_header = {
        "schema": TRACE_SCHEMA,
        "pair_index": metrics.pair_index,
        "variant": metrics.variant,
        "seed": raw["seed"],
        "clock": "monotonic_nanoseconds_relative_to_measurement_start",
        "logical_id_derivation": LOGICAL_ID_DERIVATION,
        "transaction_hash_source": "iroha_data_model::transaction::SignedTransaction::hash",
    }
    for name, expected in expected_header.items():
        if trace[name] != expected or type(trace[name]) is not type(expected):
            _fail(f"{label} transaction trace.{name} does not match its declared provenance")
    rows = _require_list(trace["transactions"], f"{label} transaction trace.transactions")
    warmup_count = math.ceil(Fraction(warmup_ns, 1) / period)
    measurement_count = math.ceil(Fraction(measurement_ns, 1) / period)
    if len(rows) > MAX_TRANSACTION_TRACE_ROWS or len(rows) != warmup_count + measurement_count:
        _fail(f"{label} transaction trace has missing or extra scheduled requests or exceeds its row bound")

    events = {phase: {name: [] for name in _COUNT_FIELDS} for phase in ("measurement", "drain")}
    warmup_counts = dict.fromkeys(_COUNT_FIELDS, 0)
    cohort_latencies: list[float] = []
    drain_rejected_count = 0
    for index, row_raw in enumerate(rows):
        row_label = f"{label} transaction trace.transactions[{index}]"
        row = _require_object(row_raw, row_label)
        _require_exact_fields(row, _TRANSACTION_FIELDS, row_label)
        is_warmup = index < warmup_count
        cohort = "warmup" if is_warmup else "measurement"
        sequence = index + 1 if is_warmup else index - warmup_count + 1
        if row["cohort"] != cohort or type(row["sequence"]) is not int or row["sequence"] != sequence:
            _fail(f"{row_label} has a missing, duplicate, or reordered scheduled cohort/sequence")
        logical_id = hashlib.sha256(f"{raw['seed']}:{cohort}:{sequence}".encode("ascii")).hexdigest()
        if row["logical_id"] != logical_id:
            _fail(f"{row_label}.logical_id does not match the pair-seeded request")
        tx_hash = _require_text(row["hash"], f"{row_label}.hash")
        if _TRANSACTION_HASH_RE.fullmatch(tx_hash) is None:
            _fail(f"{row_label}.hash is not a canonical signed transaction hash")
        if tx_hash in seen_transaction_hashes:
            _fail(f"{row_label}.hash duplicates a transaction identity in this matrix")
        seen_transaction_hashes.add(tx_hash)
        phase_start = -(warmup_ns + drain_ns) if is_warmup else 0
        phase_end = -drain_ns if is_warmup else measurement_ns
        scheduled = phase_start + math.floor((sequence - 1) * period)
        if _offset_ns(row["scheduled_offset_ns"], f"{row_label}.scheduled_offset_ns") != scheduled:
            _fail(f"{row_label} reschedules the fixed open-loop offer")
        offer = _offset_ns(row["offer_offset_ns"], f"{row_label}.offer_offset_ns")
        lag = _offset_ns(row["submission_lag_ns"], f"{row_label}.submission_lag_ns")
        if lag != offer - scheduled or not 0 <= lag <= lag_bound or offer >= phase_end:
            _fail(f"{row_label} misses the fixed submission-lag bound or offer window")
        ack = _require_object(row["acknowledgment"], f"{row_label}.acknowledgment")
        _require_exact_fields(ack, _ACK_FIELDS, f"{row_label}.acknowledgment")
        ack_offset = _offset_ns(ack["offset_ns"], f"{row_label}.acknowledgment.offset_ns")
        deadline = 0 if is_warmup else measurement_ns + drain_ns
        if ack["hash"] != tx_hash or ack["status"] not in ("Accepted", "Rejected"):
            _fail(f"{row_label} acknowledgment has an unknown status or mismatched transaction identity")
        if ack_offset < offer or ack_offset > deadline or (is_warmup and ack_offset == 0):
            _fail(f"{row_label} acknowledgment is before offer, after the drain deadline, or leaves warmup undrained")
        accepted = ack["status"] == "Accepted"
        applied_offset: int | None = None
        latency = 0.0
        if accepted:
            if ack["rejection"] is not None:
                _fail(f"{row_label} Accepted acknowledgment must have null rejection")
            applied = _require_object(row["applied"], f"{row_label}.applied")
            _require_exact_fields(applied, _APPLIED_FIELDS, f"{row_label}.applied")
            if (
                applied["hash"] != tx_hash or applied["scope"] != "global"
                or applied["resolved_from"] != "state" or applied["status"] != "Applied"
            ):
                _fail(f"{row_label} accepted transaction lacks exact authoritative global StateApplied")
            height = _require_int(applied["block_height"], f"{row_label}.applied.block_height", minimum=1)
            if height > (1 << 64) - 1:
                _fail(f"{row_label}.applied.block_height exceeds the authoritative u64 height bound")
            applied_offset = _offset_ns(applied["offset_ns"], f"{row_label}.applied.offset_ns")
            # A response may race a state observation; do not reorder either event.
            if applied_offset <= offer or applied_offset > deadline or (is_warmup and applied_offset == 0):
                _fail(f"{row_label} StateApplied is before offer, after the drain deadline, or leaves warmup undrained")
            latency = (applied_offset - offer) / 1_000_000
        else:
            _require_text(ack["rejection"], f"{row_label}.acknowledgment.rejection")
            if row["applied"] is not None:
                _fail(f"{row_label} admission rejection cannot also claim StateApplied")
        if is_warmup:
            warmup_counts["offered_count"] += 1
            warmup_counts["accepted_count"] += int(accepted)
            warmup_counts["committed_count"] += int(accepted)
            continue
        events["measurement"]["offered_count"].append((offer, sequence, 0.0))
        if not accepted and ack_offset >= measurement_ns:
            drain_rejected_count += 1
        if accepted:
            ack_phase = "measurement" if ack_offset < measurement_ns else "drain"
            events[ack_phase]["accepted_count"].append((ack_offset, sequence, 0.0))
            assert applied_offset is not None
            applied_phase = "measurement" if applied_offset < measurement_ns else "drain"
            events[applied_phase]["committed_count"].append((applied_offset, sequence, latency))
            cohort_latencies.append(latency)

    warmup = _require_object(raw["warmup"], f"{label}.warmup")
    _require_exact_fields(warmup, _COUNT_FIELDS, f"{label}.warmup")
    for name, expected in warmup_counts.items():
        if _require_int(warmup[name], f"{label}.warmup.{name}") != expected:
            _fail(f"{label}.warmup.{name} disagrees with its separate fully drained cohort")
    _reconcile_event_samples(raw["samples"], events["measurement"], label=f"{label}.samples", final_inclusive=False)

    drain = _require_object(raw["drain"], f"{label}.drain")
    _require_exact_fields(drain, {"summary", "samples"}, f"{label}.drain")
    summary = _require_object(drain["summary"], f"{label}.drain.summary")
    _require_exact_fields(summary, _SUMMARY_FIELDS, f"{label}.drain.summary")
    samples = _require_list(drain["samples"], f"{label}.drain.samples")
    if not samples or len(samples) > MAX_TRANSACTION_TRACE_ROWS:
        _fail(f"{label}.drain.samples must contain bounded, complete drain observations")
    max_interval = max(
        _nanoseconds(sample["end_offset_seconds"], "measurement interval end")
        - _nanoseconds(sample["start_offset_seconds"], "measurement interval start")
        for sample in raw["samples"]
    )
    totals = dict.fromkeys(_COUNT_FIELDS, 0)
    drain_maxima = dict.fromkeys(_BUDGET_FIELDS, 0)
    previous_end = measurement_ns
    for index, sample_raw in enumerate(samples):
        sample_label = f"{label}.drain.samples[{index}]"
        sample = _require_object(sample_raw, sample_label)
        _require_exact_fields(sample, _SAMPLE_FIELDS, sample_label)
        if type(sample["sequence"]) is not int or sample["sequence"] != index + 1:
            _fail(f"{sample_label}.sequence is unordered")
        start = _nanoseconds(sample["start_offset_seconds"], f"{sample_label}.start_offset_seconds")
        end = _nanoseconds(sample["end_offset_seconds"], f"{sample_label}.end_offset_seconds")
        if start != previous_end or not 0 < end - start <= max_interval or end > measurement_ns + drain_ns:
            _fail(f"{sample_label} is unordered, leaves a drain gap, or weakens observation cadence")
        previous_end = end
        for name in _COUNT_FIELDS:
            totals[name] += _require_int(sample[name], f"{sample_label}.{name}")
        latencies = _require_list(sample["commit_latencies_ms"], f"{sample_label}.commit_latencies_ms")
        if len(latencies) != sample["committed_count"]:
            _fail(f"{sample_label} must contain one latency for every committed transaction")
        for latency in latencies:
            _require_number(latency, f"{sample_label}.commit_latencies_ms", strictly_positive=True)
        for name in _BUDGET_FIELDS:
            observed = _require_int(sample[name.removesuffix("_max")], f"{sample_label}.{name}")
            if observed > budgets[name]:
                _fail(f"{sample_label} exceeds {name} budget during drain")
            drain_maxima[name] = max(drain_maxima[name], observed)
    if previous_end != measurement_ns + drain_ns:
        _fail(f"{label}.drain.samples do not exactly cover the bounded drain window")
    for name, expected in (totals | drain_maxima).items():
        if _require_int(summary[name], f"{label}.drain.summary.{name}") != expected:
            _fail(f"{label}.drain.summary.{name} is inconsistent with raw drain samples")
    _reconcile_event_samples(samples, events["drain"], label=f"{label}.drain.samples", final_inclusive=True)
    if len(cohort_latencies) != metrics.accepted_count + totals["accepted_count"]:
        _fail(f"{label} accepted cohort accounting disagrees with complete StateApplied observations")
    allocation = select_run_budget(controls.budget, metrics.pair_index, metrics.variant)
    journal_binding = _require_ref(raw['artifacts']['collector_journal'], controls,
        f'{label} collector journal', expected_label=allocation.journal.label)
    scope_index = (metrics.pair_index - 1) * 2 + int(metrics.variant == 'four_lane')
    _reconcile_journal_trace(controls.read(journal_binding, max_bytes=MAX_BUNDLE_FILE_BYTES), rows,
        pair_index=metrics.pair_index, variant=metrics.variant, seed=raw['seed'],
        geometry=controls.admission.runs[scope_index].geometry, submission_lag_bound_ns=lag_bound)
    return replace(
        metrics,
        p95_latency_ms=_nearest_rank_p95(cohort_latencies),
        latency_sample_count=len(cohort_latencies),
        latencies_ms=tuple(cohort_latencies),
        maxima={name: max(metrics.maxima[name], drain_maxima[name]) for name in _BUDGET_FIELDS},
        cohort_accepted_count=len(cohort_latencies),
        drain_accepted_count=totals["accepted_count"],
        drain_committed_count=totals["committed_count"],
        drain_rejected_count=drain_rejected_count,
    )


def _validate_raw_run(
    raw: Any,
    *,
    label: str,
    pair_index: int,
    variant: str,
    active_lanes: int,
    seed: str,
    identity: dict[str, Any],
    workload: dict[str, Any],
    budgets: dict[str, Any],
    controls: EvidenceControls,
    seen_support_paths: set[ControlBinding],
    referenced_paths: set[str],
) -> RunMetrics:
    run = _require_object(raw, label)
    _require_exact_fields(run, _RAW_RUN_FIELDS, label)
    if run["schema"] != RUN_SCHEMA:
        _fail(f"{label}.schema must be {RUN_SCHEMA!r}")
    if run["pair_index"] != pair_index:
        _fail(f"{label}.pair_index does not match its manifest pair")
    if run["variant"] != variant:
        _fail(f"{label}.variant does not match its manifest variant")
    if _require_int(run["active_execution_lanes"], f"{label}.active_execution_lanes", minimum=1) != active_lanes:
        _fail(f"{label}.active_execution_lanes does not match the required variant")
    if run["seed"] != seed:
        _fail(f"{label}.seed does not match the deterministic pair seed")
    validate_identity(run["identity_before"], f"{label}.identity_before")
    if run["identity_before"] != identity:
        _fail(f"{label}.identity_before drifted from the pinned bundle identity")
    validate_identity(run["identity_after"], f"{label}.identity_after")
    if run["identity_after"] != identity:
        _fail(f"{label}.identity_after drifted from the pinned bundle identity")

    lane_ids_raw = _require_list(run["execution_lane_ids"], f"{label}.execution_lane_ids")
    if len(lane_ids_raw) != active_lanes:
        _fail(
            f"{label}.execution_lane_ids must contain exactly {active_lanes} "
            "active execution lanes"
        )
    lane_ids = tuple(
        _require_text(item, f"{label}.execution_lane_ids[{index}]")
        for index, item in enumerate(lane_ids_raw)
    )
    if len(set(lane_ids)) != len(lane_ids):
        _fail(f"{label}.execution_lane_ids contains a duplicate lane")
    if list(lane_ids) != sorted(lane_ids):
        _fail(f"{label}.execution_lane_ids must use canonical sorted order")

    artifacts = _require_object(run["artifacts"], f"{label}.artifacts")
    _require_exact_fields(artifacts, _RUN_ARTIFACT_FIELDS, f"{label}.artifacts")
    allocation = select_run_budget(controls.budget, pair_index, variant)
    artifact_labels = {
        "transaction_trace": allocation.run.transaction_trace.label,
        "collector_journal": allocation.journal.label,
        "canonical_proof": allocation.run.canonical_proof.label,
        **{role: f"pair-{pair_index:02}.{variant}.{role}" for role in _SUPPORT_ROLES},
    }
    artifact_paths: dict[str, ControlBinding] = {}
    for name in sorted(_RUN_ARTIFACT_FIELDS):
        path = _require_ref(
            artifacts[name],
            controls,
            f"{label}.artifacts.{name}", expected_label=artifact_labels[name],
            referenced_paths=referenced_paths,
        )
        if path in seen_support_paths:
            _fail(f"{label}.artifacts.{name} duplicates support evidence from another run")
        seen_support_paths.add(path)
        artifact_paths[name] = path
    nexus_manifest = _require_object(
        load_json(
            artifact_paths["nexus_load_test_manifest"],
            f"{label} Nexus lane-load manifest", controls=controls,
            max_bytes=MAX_METADATA_JSON_BYTES,
        ),
        f"{label} Nexus lane-load manifest",
    )
    if _require_int(nexus_manifest.get("version"), f"{label} Nexus lane-load manifest.version", minimum=1) != 1:
        _fail(f"{label} Nexus lane-load manifest must have version 1")
    if nexus_manifest.get("lanes") != list(lane_ids):
        _fail(f"{label} Nexus lane-load manifest lanes do not match active execution lanes")
    if nexus_manifest.get("workload_seed") != seed:
        _fail(f"{label} Nexus lane-load manifest workload_seed does not match the pair seed")
    nexus_inputs = _require_object(
        nexus_manifest.get("inputs"),
        f"{label} Nexus lane-load manifest inputs",
    )
    _require_exact_fields(
        nexus_inputs,
        _NEXUS_LOAD_INPUT_FIELDS,
        f"{label} Nexus lane-load manifest inputs",
    )
    _require_text(
        nexus_inputs["lifecycle_file"],
        f"{label} Nexus lane-load manifest inputs.lifecycle_file",
    )

    raw_workload = _require_object(run["workload"], f"{label}.workload")
    _require_exact_fields(raw_workload, _WORKLOAD_FIELDS, f"{label}.workload")
    if raw_workload != workload:
        _fail(f"{label}.workload drifted from the pinned bundle workload")

    status = _require_object(run["status"], f"{label}.status")
    _require_exact_fields(status, _STATUS_FIELDS, f"{label}.status")
    if status["outcome"] != "passed":
        _fail(f"{label}.status.outcome must be 'passed'")
    if status["skipped"] is not False:
        _fail(f"{label}.status.skipped must be false")
    if status["failure"] is not None:
        _fail(f"{label}.status.failure must be null for a passed run")

    summary = _require_object(run["summary"], f"{label}.summary")
    _require_exact_fields(summary, _SUMMARY_FIELDS, f"{label}.summary")
    offered_count = _require_int(summary["offered_count"], f"{label}.summary.offered_count", minimum=1)
    accepted_count = _require_int(summary["accepted_count"], f"{label}.summary.accepted_count")
    committed_count = _require_int(
        summary["committed_count"],
        f"{label}.summary.committed_count",
        minimum=1,
    )
    if accepted_count > offered_count:
        _fail(f"{label}.summary.accepted_count exceeds offered_count")
    if committed_count > accepted_count:
        _fail(f"{label}.summary.committed_count exceeds accepted_count")

    samples = _require_list(run["samples"], f"{label}.samples")
    if len(samples) > MAX_TRANSACTION_TRACE_ROWS:
        _fail(f"{label}.samples exceeds the bounded interval sample count")
    minimum_intervals = workload["min_interval_samples"]
    if len(samples) < minimum_intervals:
        _fail(
            f"{label}.samples has weak interval sample count: "
            f"{len(samples)} < {minimum_intervals}"
        )

    count_totals = {"offered_count": 0, "accepted_count": 0, "committed_count": 0}
    maxima = {
        "queue_depth_max": 0,
        "index_entries_max": 0,
        "memory_bytes_max": 0,
        "disk_bytes_max": 0,
    }
    sample_to_maximum = {
        "queue_depth": "queue_depth_max",
        "index_entries": "index_entries_max",
        "memory_bytes": "memory_bytes_max",
        "disk_bytes": "disk_bytes_max",
    }
    latencies: list[float] = []
    previous_end = 0.0
    for offset, sample_raw in enumerate(samples, start=1):
        sample_label = f"{label}.samples[{offset - 1}]"
        sample = _require_object(sample_raw, sample_label)
        _require_exact_fields(sample, _SAMPLE_FIELDS, sample_label)
        if sample["sequence"] != offset:
            _fail(f"{sample_label}.sequence must be {offset}")
        start = _require_number(
            sample["start_offset_seconds"],
            f"{sample_label}.start_offset_seconds",
            minimum=0,
        )
        end = _require_number(
            sample["end_offset_seconds"],
            f"{sample_label}.end_offset_seconds",
            strictly_positive=True,
        )
        if not _same_number(start, previous_end):
            _fail(f"{sample_label} is unordered or leaves a measurement interval gap")
        if end <= start:
            _fail(f"{sample_label}.end_offset_seconds must exceed its start")
        previous_end = end

        for count_name in count_totals:
            count = _require_int(sample[count_name], f"{sample_label}.{count_name}")
            count_totals[count_name] += count

        latency_values = _require_list(
            sample["commit_latencies_ms"],
            f"{sample_label}.commit_latencies_ms",
        )
        interval_committed = sample["committed_count"]
        if len(latency_values) != interval_committed:
            _fail(
                f"{sample_label}.commit_latencies_ms must contain one latency "
                "for every committed transaction"
            )
        for latency_index, latency_raw in enumerate(latency_values):
            latencies.append(
                _require_number(
                    latency_raw,
                    f"{sample_label}.commit_latencies_ms[{latency_index}]",
                    strictly_positive=True,
                )
            )

        for sample_name, maximum_name in sample_to_maximum.items():
            observed = _require_int(sample[sample_name], f"{sample_label}.{sample_name}")
            budget = budgets[maximum_name]
            if observed > budget:
                _fail(
                    f"{sample_label}.{sample_name} exceeds {maximum_name} budget: "
                    f"{observed} > {budget}"
                )
            maxima[maximum_name] = max(maxima[maximum_name], observed)

    measurement_seconds = workload["measurement_seconds"]
    if not _same_number(previous_end, measurement_seconds):
        _fail(f"{label}.samples do not exactly cover measurement_seconds")
    for count_name, computed in count_totals.items():
        if summary[count_name] != computed:
            _fail(
                f"{label}.summary.{count_name} is inconsistent with raw samples: "
                f"recorded={summary[count_name]}, computed={computed}"
            )
    for maximum_name, computed in maxima.items():
        recorded = _require_int(summary[maximum_name], f"{label}.summary.{maximum_name}")
        if recorded != computed:
            _fail(
                f"{label}.summary.{maximum_name} is inconsistent with raw samples: "
                f"recorded={recorded}, computed={computed}"
            )
        if recorded > budgets[maximum_name]:
            _fail(
                f"{label}.summary.{maximum_name} exceeds its budget: "
                f"{recorded} > {budgets[maximum_name]}"
            )

    minimum_latencies = workload["min_latency_samples"]
    if len(latencies) < minimum_latencies:
        _fail(
            f"{label} has weak latency sample count: "
            f"{len(latencies)} < {minimum_latencies}"
        )
    if len(latencies) != committed_count:
        _fail(f"{label} latency sample count does not equal committed_count")

    actual_offered_tps = offered_count / measurement_seconds
    target_offered_tps = workload["offered_load_tps"]
    deviation = abs(actual_offered_tps - target_offered_tps) / target_offered_tps
    if deviation > workload["max_offered_load_deviation_fraction"]:
        _fail(
            f"{label} actual offered load deviates from target by {deviation:.6f}, "
            f"over {workload['max_offered_load_deviation_fraction']:.6f}"
        )

    return RunMetrics(
        pair_index=pair_index,
        variant=variant,
        lane_ids=lane_ids,
        offered_count=offered_count,
        accepted_count=accepted_count,
        committed_count=committed_count,
        throughput_tps=committed_count / measurement_seconds,
        p95_latency_ms=_nearest_rank_p95(latencies),
        interval_sample_count=len(samples),
        latency_sample_count=len(latencies),
        latencies_ms=tuple(latencies),
        maxima=maxima,
    )


def _validate_admitted_evidence(
    manifest_path: Path,
    *,
    controls: EvidenceControls,
    expected_source_revision: str | None = None,
    expected_workspace_source_sha256: str | None = None,
    expected_validator_sha256: str | None = None,
    expected_trial_harness_sha256: str | None = None,
    expected_configuration_sha256: str | None = None,
    expected_irohad_sha256: str | None = None,
    expected_iroha_cli_sha256: str | None = None,
    expected_repository_root: Path | None = None,
) -> dict[str, Any]:
    """Validate *manifest_path* and return deterministic, recomputed metrics.

    The optional expected values bind an otherwise self-consistent benchmark
    bundle to the exact release candidate, operator-approved harness/config,
    measured binaries, and retained tooling which consume it. They are used by
    the source-sealed release corridor before any long-running network gate
    starts.
    """

    for value, label in (
        (expected_workspace_source_sha256, "expected workspace source"),
        (expected_validator_sha256, "expected validator"),
        (expected_trial_harness_sha256, "expected trial harness"),
        (expected_configuration_sha256, "expected configuration"),
        (expected_irohad_sha256, "expected irohad"),
        (expected_iroha_cli_sha256, "expected iroha CLI"),
    ):
        if value is not None:
            _require_digest(value, label)
    if expected_repository_root is not None:
        _fail('repository path trust is retired; supply independently pinned static controls')
    if (not manifest_path.is_absolute() or str(manifest_path) != os.path.abspath(manifest_path)
            or manifest_path.name != "scaling_evidence.json"):
        _fail('evidence manifest must use the absolute canonical scaling_evidence.json path')
    root = manifest_path.parent
    if root != controls.owner._root:
        _fail('manifest root differs from retained experiment root')
    manifest_binding = controls.binding(controls.budget.control_budgets[0].label)
    if manifest_binding.path != 'scaling_evidence.json':
        _fail('manifest must bind the canonical scaling_evidence.json control')
    referenced_paths: set[str] = set()
    manifest = _require_object(load_json(manifest_binding, "evidence manifest", controls=controls,
                                        max_bytes=MAX_METADATA_JSON_BYTES), "evidence manifest")
    _require_exact_fields(manifest, _MANIFEST_FIELDS, "evidence manifest")
    if manifest["schema"] != EVIDENCE_SCHEMA:
        _fail(f"evidence manifest.schema must be {EVIDENCE_SCHEMA!r}")
    _require_timestamp(manifest["generated_at_utc"], "evidence manifest.generated_at_utc")
    if _require_int(manifest["pair_count"], "evidence manifest.pair_count", minimum=1) != EXPECTED_PAIR_COUNT:
        _fail(f"evidence manifest.pair_count must be exactly {EXPECTED_PAIR_COUNT}")

    namespace = _require_text(manifest["seed_namespace"], "evidence manifest.seed_namespace")
    if _SEED_NAMESPACE_RE.fullmatch(namespace) is None:
        _fail("evidence manifest.seed_namespace has invalid characters or length")
    if manifest["seed_derivation"] != SEED_DERIVATION:
        _fail(f"evidence manifest.seed_derivation must be {SEED_DERIVATION!r}")

    identity_path = _require_ref(
        manifest["identity"],
        controls,
        "evidence manifest.identity", expected_label="identity",
        referenced_paths=referenced_paths,
    )
    identity = validate_identity(load_json(identity_path, "pinned identity", controls=controls, max_bytes=MAX_METADATA_JSON_BYTES), "pinned identity")
    if identity["software"]["irohad_sha256"] != controls.admission.expected_executable_sha256:
        _fail('pinned identity differs from the independently measured executable')
    if (
        expected_source_revision is not None
        and identity["software"]["source_revision"] != expected_source_revision
    ):
        _fail(
            "pinned identity software.source_revision does not match the "
            "expected release source"
        )
    if (
        expected_workspace_source_sha256 is not None
        and identity["software"]["workspace_source_sha256"]
        != expected_workspace_source_sha256
    ):
        _fail(
            "pinned identity software.workspace_source_sha256 does not match "
            "the expected sealed workspace"
        )
    if (
        expected_irohad_sha256 is not None
        and identity["software"]["irohad_sha256"] != expected_irohad_sha256
    ):
        _fail(
            "pinned identity software.irohad_sha256 does not match the "
            "expected measured binary"
        )
    if (
        expected_iroha_cli_sha256 is not None
        and identity["software"]["iroha_cli_sha256"]
        != expected_iroha_cli_sha256
    ):
        _fail(
            "pinned identity software.iroha_cli_sha256 does not match the "
            "expected measured binary"
        )
    config_path = _require_ref(
        manifest["configuration"],
        controls,
        "evidence manifest.configuration", expected_label="configuration",
        referenced_paths=referenced_paths,
    )
    if config_path.sha256 != identity["software"]["nexus_config_sha256"]:
        _fail("configuration artifact does not match identity.software.nexus_config_sha256")
    if (
        expected_configuration_sha256 is not None
        and config_path.sha256 != expected_configuration_sha256
    ):
        _fail(
            "evidence manifest.configuration does not match the expected "
            "release configuration"
        )

    workload = _require_object(manifest["workload"], "evidence manifest.workload")
    _require_exact_fields(workload, _WORKLOAD_FIELDS, "evidence manifest.workload")
    workload_values = {
        "offered_load_tps": _require_number(
            workload["offered_load_tps"],
            "evidence manifest.workload.offered_load_tps",
            strictly_positive=True,
        ),
        "warmup_seconds": _require_number(
            workload["warmup_seconds"],
            "evidence manifest.workload.warmup_seconds",
            minimum=0,
        ),
        "measurement_seconds": _require_number(
            workload["measurement_seconds"],
            "evidence manifest.workload.measurement_seconds",
            strictly_positive=True,
        ),
        "drain_seconds": _require_number(
            workload["drain_seconds"],
            "evidence manifest.workload.drain_seconds",
            strictly_positive=True,
        ),
        "max_submission_lag_ms": _require_number(
            workload["max_submission_lag_ms"],
            "evidence manifest.workload.max_submission_lag_ms",
            minimum=0,
        ),
        "min_interval_samples": _require_int(
            workload["min_interval_samples"],
            "evidence manifest.workload.min_interval_samples",
            minimum=MIN_INTERVAL_SAMPLES,
        ),
        "min_latency_samples": _require_int(
            workload["min_latency_samples"],
            "evidence manifest.workload.min_latency_samples",
            minimum=MIN_LATENCY_SAMPLES,
        ),
        "max_offered_load_deviation_fraction": _require_number(
            workload["max_offered_load_deviation_fraction"],
            "evidence manifest.workload.max_offered_load_deviation_fraction",
            minimum=0,
        ),
    }
    if not _same_number(
        workload_values["max_offered_load_deviation_fraction"],
        MAX_OFFERED_LOAD_DEVIATION_FRACTION,
    ):
        _fail(
            "evidence manifest.workload.max_offered_load_deviation_fraction "
            f"must be exactly {MAX_OFFERED_LOAD_DEVIATION_FRACTION}"
        )
    _schedule(workload_values)

    _, warmup_ns, measurement_ns, drain_ns, _ = _schedule(workload_values)
    for scope in controls.admission.runs:
        if (scope.geometry.warmup_ns, scope.geometry.measurement_ns, scope.geometry.drain_ns) != (warmup_ns, measurement_ns, drain_ns):
            _fail('workload timing differs from the independently admitted Clock geometry')

    budgets = _require_object(manifest["budgets"], "evidence manifest.budgets")
    _require_exact_fields(budgets, _BUDGET_FIELDS, "evidence manifest.budgets")
    budget_values = {
        field: _require_int(value, f"evidence manifest.budgets.{field}", minimum=1)
        for field, value in budgets.items()
    }

    scopes = _require_object(manifest["observation_scope"], "evidence manifest.observation_scope")
    _require_exact_fields(scopes, _SCOPE_FIELDS, "evidence manifest.observation_scope")
    for field in sorted(_SCOPE_FIELDS):
        _require_text(scopes[field], f"evidence manifest.observation_scope.{field}")

    thresholds = _require_object(manifest["thresholds"], "evidence manifest.thresholds")
    _require_exact_fields(thresholds, _THRESHOLD_FIELDS, "evidence manifest.thresholds")
    throughput_threshold = _require_number(
        thresholds["min_four_lane_throughput_ratio"],
        "evidence manifest.thresholds.min_four_lane_throughput_ratio",
        strictly_positive=True,
    )
    latency_threshold = _require_number(
        thresholds["max_four_lane_p95_latency_ratio"],
        "evidence manifest.thresholds.max_four_lane_p95_latency_ratio",
        strictly_positive=True,
    )
    if not _same_number(throughput_threshold, MIN_THROUGHPUT_RATIO):
        _fail(
            "evidence manifest cannot weaken or change the throughput threshold; "
            f"expected {MIN_THROUGHPUT_RATIO}"
        )
    if not _same_number(latency_threshold, MAX_P95_LATENCY_RATIO):
        _fail(
            "evidence manifest cannot weaken or change the p95 latency threshold; "
            f"expected {MAX_P95_LATENCY_RATIO}"
        )

    trial_harness_path = _require_ref(
        manifest["trial_harness"],
        controls,
        "evidence manifest.trial_harness", expected_label="trial_harness",
        referenced_paths=referenced_paths,
    )
    if (
        expected_trial_harness_sha256 is not None
        and trial_harness_path.sha256 != expected_trial_harness_sha256
    ):
        _fail(
            "evidence manifest.trial_harness does not match the expected "
            "operator-approved harness"
        )
    validator_path = _require_ref(
        manifest["validator"],
        controls,
        "evidence manifest.validator", expected_label="validator",
        referenced_paths=referenced_paths,
    )
    if (
        expected_validator_sha256 is not None
        and validator_path.sha256 != expected_validator_sha256
    ):
        _fail(
            "evidence manifest.validator does not match the expected retained "
            "validator"
        )
    tooling = _require_list(manifest["tooling"], "evidence manifest.tooling")
    if len(tooling) != len(REQUIRED_TOOLING):
        _fail("evidence manifest.tooling must contain the three required tooling artifacts")
    for index, (entry_raw, expected) in enumerate(zip(tooling, REQUIRED_TOOLING)):
        role, source_path = expected
        label = f"evidence manifest.tooling[{index}]"
        entry = _require_object(entry_raw, label)
        _require_exact_fields(entry, _TOOL_FIELDS, label)
        if entry["role"] != role or entry["source_path"] != source_path:
            _fail(f"{label} does not identify required tool {role}:{source_path}")
        artifact_path = _require_ref(
            entry["artifact"],
            controls,
            f"{label}.artifact", expected_label=role,
            referenced_paths=referenced_paths,
        )

    runs = _require_list(manifest["runs"], "evidence manifest.runs")
    expected_run_count = EXPECTED_PAIR_COUNT * 2
    if len(runs) != expected_run_count:
        _fail(
            "evidence manifest.runs must contain exactly ten entries "
            "(five complete one-lane/four-lane pairs)"
        )

    expected_order = [
        (pair_index, variant, active_lanes)
        for pair_index in range(1, EXPECTED_PAIR_COUNT + 1)
        for variant, active_lanes in (("one_lane", 1), ("four_lane", 4))
    ]
    seen_raw_paths: set[ControlBinding] = set()
    seen_log_paths: set[ControlBinding] = set()
    seen_support_paths: set[ControlBinding] = set()
    metrics: list[RunMetrics] = []
    raw_runs: list[dict[str, Any]] = []
    for sequence, (entry_raw, expected) in enumerate(
        zip(runs, expected_order),
        start=1,
    ):
        pair_index, variant, active_lanes = expected
        label = f"evidence manifest.runs[{sequence - 1}]"
        entry = _require_object(entry_raw, label)
        _require_exact_fields(entry, _RUN_ENTRY_FIELDS, label)
        actual_identity = (
            _require_int(entry["pair_index"], f"{label}.pair_index", minimum=1),
            entry["variant"],
            _require_int(entry["active_execution_lanes"], f"{label}.active_execution_lanes", minimum=1),
        )
        if actual_identity != expected:
            _fail(
                "evidence manifest.runs has missing, duplicate, or unordered pairs; "
                f"entry {sequence} must be {expected}, got {actual_identity}"
            )
        if _require_int(entry["sequence"], f"{label}.sequence", minimum=1) != sequence:
            _fail(f"{label}.sequence must be {sequence}")
        seed = derive_seed(namespace, pair_index)
        if entry["seed"] != seed:
            _fail(f"{label}.seed does not match deterministic pair derivation")
        if entry["status"] != "passed":
            _fail(f"{label}.status must be 'passed'")
        if entry["skipped"] is not False:
            _fail(f"{label}.skipped must be false")
        if entry["exit_code"] != 0 or type(entry["exit_code"]) is not int:
            _fail(f"{label}.exit_code must be integer zero")

        raw_path = _require_ref(
            entry["raw_samples"],
            controls,
            f"{label}.raw_samples", expected_label=select_run_budget(
                controls.budget, pair_index, variant).run.raw_run.label,
            referenced_paths=referenced_paths,
        )
        log_path = _require_ref(
            entry["command_log"],
            controls,
            f"{label}.command_log", expected_label=select_run_budget(
                controls.budget, pair_index, variant).run.trial_log.label,
            referenced_paths=referenced_paths,
        )
        if raw_path in seen_raw_paths:
            _fail(f"{label}.raw_samples duplicates another run path")
        seen_raw_paths.add(raw_path)
        if log_path in seen_log_paths:
            _fail(f"{label}.command_log duplicates another run path")
        seen_log_paths.add(log_path)
        if raw_path == log_path:
            _fail(f"{label} cannot use the raw sample file as its command log")
        raw = load_json(raw_path, f"{label} raw samples", controls=controls, max_bytes=MAX_RAW_RUN_JSON_BYTES)
        raw_runs.append(raw)
        metrics.append(
            _validate_raw_run(
                raw,
                label=f"pair {pair_index} {variant}",
                pair_index=pair_index,
                variant=variant,
                active_lanes=active_lanes,
                seed=seed,
                identity=identity,
                workload=workload_values,
                budgets=budget_values,
                controls=controls,
                seen_support_paths=seen_support_paths,
                referenced_paths=referenced_paths,
            )
        )

    pairs: list[dict[str, Any]] = []
    one_runs: list[RunMetrics] = []
    four_runs: list[RunMetrics] = []
    baseline_one_lane_ids: tuple[str, ...] | None = None
    baseline_four_lane_ids: tuple[str, ...] | None = None
    baseline_offered_count: int | None = None
    seen_transaction_hashes: set[str] = set()
    for pair_index in range(1, EXPECTED_PAIR_COUNT + 1):
        one = metrics[(pair_index - 1) * 2]
        four = metrics[(pair_index - 1) * 2 + 1]
        if one.offered_count != four.offered_count:
            _fail(
                f"pair {pair_index} offered load is not matched: "
                f"one_lane={one.offered_count}, four_lane={four.offered_count}"
            )
        if one.lane_ids[0] not in four.lane_ids:
            _fail(f"pair {pair_index} four-lane set does not contain the baseline execution lane")
        if baseline_one_lane_ids is None:
            baseline_one_lane_ids = one.lane_ids
            baseline_four_lane_ids = four.lane_ids
            baseline_offered_count = one.offered_count
        elif one.lane_ids != baseline_one_lane_ids or four.lane_ids != baseline_four_lane_ids:
            _fail(f"pair {pair_index} active execution-lane identity drifted across trials")
        if one.offered_count != baseline_offered_count:
            _fail(
                f"pair {pair_index} offered count drifted across trials: "
                f"{one.offered_count} != {baseline_offered_count}"
            )
        one = _validate_run_trace(
            raw_runs[(pair_index - 1) * 2], one,
            controls=controls, workload=workload_values, budgets=budget_values,
            seen_transaction_hashes=seen_transaction_hashes,
        )
        four = _validate_run_trace(
            raw_runs[(pair_index - 1) * 2 + 1], four,
            controls=controls, workload=workload_values, budgets=budget_values,
            seen_transaction_hashes=seen_transaction_hashes,
        )
        one = replace(one, maxima=vars_maxima(controls.owner.reconcile_run(
            pair_index, 'one_lane', raw_runs[(pair_index - 1) * 2], budget_values)))
        four = replace(four, maxima=vars_maxima(controls.owner.reconcile_run(
            pair_index, 'four_lane', raw_runs[(pair_index - 1) * 2 + 1], budget_values)))
        one_runs.append(one)
        four_runs.append(four)
        pairs.append(
            {
                "pair_index": pair_index,
                "seed": derive_seed(namespace, pair_index),
                "offered_count": one.offered_count,
                "one_lane_accepted_count": one.accepted_count,
                "four_lane_accepted_count": four.accepted_count,
                "one_lane_committed_count": one.committed_count,
                "four_lane_committed_count": four.committed_count,
                "one_lane_committed_throughput_tps": one.throughput_tps,
                "four_lane_committed_throughput_tps": four.throughput_tps,
                "one_lane_p95_latency_ms": one.p95_latency_ms,
                "four_lane_p95_latency_ms": four.p95_latency_ms,
                "one_lane_interval_samples": one.interval_sample_count,
                "four_lane_interval_samples": four.interval_sample_count,
                "one_lane_latency_samples": one.latency_sample_count,
                "four_lane_latency_samples": four.latency_sample_count,
                "one_lane_cohort_accepted_count": one.cohort_accepted_count,
                "four_lane_cohort_accepted_count": four.cohort_accepted_count,
                "one_lane_drain_accepted_count": one.drain_accepted_count,
                "four_lane_drain_accepted_count": four.drain_accepted_count,
                "one_lane_drain_committed_count": one.drain_committed_count,
                "four_lane_drain_committed_count": four.drain_committed_count,
                "one_lane_drain_rejected_count": one.drain_rejected_count,
                "four_lane_drain_rejected_count": four.drain_rejected_count,
                "one_lane_resource_maxima": one.maxima,
                "four_lane_resource_maxima": four.maxima,
            }
        )

    one_median = statistics.median(run.throughput_tps for run in one_runs)
    four_median = statistics.median(run.throughput_tps for run in four_runs)
    throughput_ratio = four_median / one_median
    one_latencies = tuple(value for run in one_runs for value in run.latencies_ms)
    four_latencies = tuple(value for run in four_runs for value in run.latencies_ms)
    one_p95 = _nearest_rank_p95(one_latencies)
    four_p95 = _nearest_rank_p95(four_latencies)
    latency_ratio = four_p95 / one_p95
    one_resource_maxima = {
        field: max(run.maxima[field] for run in one_runs)
        for field in sorted(_BUDGET_FIELDS)
    }
    four_resource_maxima = {
        field: max(run.maxima[field] for run in four_runs)
        for field in sorted(_BUDGET_FIELDS)
    }

    if throughput_ratio < MIN_THROUGHPUT_RATIO:
        _fail(
            "four-lane median committed throughput gate failed: "
            f"ratio={throughput_ratio:.12g} < {MIN_THROUGHPUT_RATIO}"
        )
    if latency_ratio > MAX_P95_LATENCY_RATIO:
        _fail(
            "four-lane pooled p95 commit latency gate failed: "
            f"ratio={latency_ratio:.12g} > {MAX_P95_LATENCY_RATIO}"
        )

    expected_files = referenced_paths | {'scaling_evidence.json'}
    if controls.admission.reported:
        report = controls.binding(controls.budget.control_budgets[1].label)
        if report.path != 'validation_report.json':
            _fail('report must bind canonical validation_report.json control')
        expected_files.add(report.path)
    actual_controls = {item.path for item in controls.admission.controls}
    if expected_files != actual_controls:
        _fail('semantic references differ from the exact admitted control inventory')

    return {
        "pair_count": EXPECTED_PAIR_COUNT,
        "run_count": expected_run_count,
        "latency_scope": "complete accepted measurement cohort, offer to client-observed global StateApplied, including drain",
        "one_lane_median_committed_throughput_tps": one_median,
        "four_lane_median_committed_throughput_tps": four_median,
        "four_to_one_median_throughput_ratio": throughput_ratio,
        "one_lane_pooled_p95_commit_latency_ms": one_p95,
        "four_lane_pooled_p95_commit_latency_ms": four_p95,
        "four_to_one_p95_latency_ratio": latency_ratio,
        "one_lane_resource_maxima": one_resource_maxima,
        "four_lane_resource_maxima": four_resource_maxima,
        "minimum_throughput_ratio": MIN_THROUGHPUT_RATIO,
        "maximum_p95_latency_ratio": MAX_P95_LATENCY_RATIO,
        "pairs": pairs,
    }


def vars_maxima(value) -> dict[str, int]:
    """Expose only the four independently replayed all-peer observations."""
    return {name: getattr(value, name) for name in _BUDGET_FIELDS}


def validate_evidence(manifest_path: Path, *, admission: ResourceAdmission, **expected) -> dict[str, Any]:
    """Require retained raw replay; canonical proof integration is still pending.

    This private integration checkpoint cannot issue release qualification.
    """
    if type(admission) is not ResourceAdmission:
        _fail('independent resource admission is mandatory')
    with ResourceExperiment(manifest_path.parent, admission.budget, admission.controls,
                            admission.runs, expected_executable_sha256=admission.expected_executable_sha256,
                            reported=admission.reported) as owner:
        controls = EvidenceControls(owner, admission)
        owner.collect_replay()
        _validate_admitted_evidence(manifest_path, controls=controls, **expected)
        # TODO: Wire the bounded Kagami proof consumer and independently pinned
        # run plan before returning metrics or publishing any release PASS.
        _fail('canonical proof verifier integration is incomplete')


def _write_report(path: Path, report: dict[str, Any]) -> None:
    """Durably publish deterministic report bytes without replacing a path."""

    final_name = path.name
    if not final_name or final_name in {".", ".."} or "\x00" in final_name:
        raise OSError(errno.EINVAL, "invalid validation report name", str(path))
    data = (json.dumps(report, indent=2, sort_keys=True) + "\n").encode("utf-8")
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        directory_flags |= os.O_NOFOLLOW
    directory_fd = os.open(path.parent, directory_flags)
    temporary_name = f".gscale-report-{secrets.token_hex(16)}"
    temporary_fd: int | None = None
    owned_inode: tuple[int, int] | None = None
    published = False

    def unlink_owned(name: str) -> bool:
        if owned_inode is None:
            return False
        try:
            metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        except FileNotFoundError:
            return False
        if (
            not stat.S_ISREG(metadata.st_mode)
            or (metadata.st_dev, metadata.st_ino) != owned_inode
        ):
            return False
        os.unlink(name, dir_fd=directory_fd)
        return True

    try:
        try:
            os.stat(final_name, dir_fd=directory_fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise FileExistsError(
                errno.EEXIST,
                "validation report destination already exists",
                str(path),
            )

        temporary_flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            temporary_flags |= os.O_NOFOLLOW
        temporary_fd = os.open(
            temporary_name,
            temporary_flags,
            0o600,
            dir_fd=directory_fd,
        )
        opened = os.fstat(temporary_fd)
        if stat.S_ISREG(opened.st_mode):
            owned_inode = (opened.st_dev, opened.st_ino)
        if owned_inode is None or opened.st_nlink != 1:
            raise OSError(errno.EIO, "validation report stage is not private")
        os.fchmod(temporary_fd, 0o600)

        remaining = memoryview(data)
        while remaining:
            written = os.write(temporary_fd, remaining)
            if written <= 0:
                raise OSError(errno.EIO, "short write while publishing validation report")
            remaining = remaining[written:]
        os.fsync(temporary_fd)
        completed = os.fstat(temporary_fd)
        if (
            not stat.S_ISREG(completed.st_mode)
            or (completed.st_dev, completed.st_ino) != owned_inode
            or completed.st_nlink != 1
            or completed.st_size != len(data)
            or stat.S_IMODE(completed.st_mode) != 0o600
        ):
            raise OSError(
                errno.EIO,
                "validation report stage changed while it was written",
            )
        os.close(temporary_fd)
        temporary_fd = None

        os.link(
            temporary_name,
            final_name,
            src_dir_fd=directory_fd,
            dst_dir_fd=directory_fd,
            follow_symlinks=False,
        )
        published = True
        os.fsync(directory_fd)
        if not unlink_owned(temporary_name):
            raise OSError(errno.EIO, "validation report stage changed before cleanup")
        os.fsync(directory_fd)

        final = os.stat(final_name, dir_fd=directory_fd, follow_symlinks=False)
        if (
            not stat.S_ISREG(final.st_mode)
            or (final.st_dev, final.st_ino) != owned_inode
            or final.st_nlink != 1
            or final.st_size != len(data)
            or stat.S_IMODE(final.st_mode) != 0o600
        ):
            raise OSError(
                errno.EIO,
                "validation report changed during atomic publication",
            )
    except BaseException:
        if temporary_fd is not None:
            try:
                os.close(temporary_fd)
            except OSError:
                pass
        cleaned = False
        for name in (temporary_name, final_name if published else ""):
            if not name:
                continue
            try:
                cleaned = unlink_owned(name) or cleaned
            except OSError:
                pass
        if cleaned:
            try:
                os.fsync(directory_fd)
            except OSError:
                pass
        raise
    finally:
        os.close(directory_fd)


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path, help="Path to scaling_evidence.json.")
    parser.add_argument(
        "--report",
        type=Path,
        help="Write a machine-readable pass/fail validation report.",
    )
    parser.add_argument(
        "--expected-source-revision",
        help="Require the pinned identity to name this exact release revision.",
    )
    parser.add_argument(
        "--expected-workspace-source-sha256",
        help="Require the pinned identity to name this exact sealed workspace digest.",
    )
    parser.add_argument(
        "--expected-validator-sha256",
        help="Require the archived validator to match this exact retained digest.",
    )
    parser.add_argument(
        "--expected-trial-harness-sha256",
        help="Require the archived trial harness to match this approved digest.",
    )
    parser.add_argument(
        "--expected-configuration-sha256",
        help="Require the archived Nexus configuration to match this approved digest.",
    )
    parser.add_argument(
        "--expected-irohad-sha256",
        help="Require the pinned identity to name this measured irohad digest.",
    )
    parser.add_argument(
        "--expected-iroha-cli-sha256",
        help="Require the pinned identity to name this measured iroha CLI digest.",
    )
    parser.add_argument(
        "--expected-repository-root",
        type=Path,
        help="Require archived workload tools to match this retained source root.",
    )
    parser.add_argument("--quiet", action="store_true", help="Suppress the human summary.")
    args = parser.parse_args(argv)
    if (
        args.expected_source_revision is not None
        and _REVISION_RE.fullmatch(args.expected_source_revision) is None
    ):
        parser.error("--expected-source-revision must be lowercase 40- or 64-hex")
    for name in (
        "expected_workspace_source_sha256",
        "expected_validator_sha256",
        "expected_trial_harness_sha256",
        "expected_configuration_sha256",
        "expected_irohad_sha256",
        "expected_iroha_cli_sha256",
    ):
        value = getattr(args, name)
        if value is not None and _DIGEST_RE.fullmatch(value) is None:
            parser.error(f"--{name.replace('_', '-')} must be lowercase SHA-256")
    return args


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    # TODO: Bind the launcher's inherited authority descriptor and compiled
    # canonical proof verifier before this entrypoint can publish any report.
    if not args.quiet:
        print('[g-scale] FAIL: mandatory launcher and canonical proof integration is incomplete', file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
