"""Pure, bounded admission for the ten-run raw resource evidence experiment.

The caller supplies trusted geometry, actual pinned static sizes, and explicit
maximum sizes for every dynamic control file. This module performs no filesystem,
process, network, sampler, or replay work. An admitted reservation is not proof
that writers enforce it or that a bundle's typed resource subtree was scanned.
Those owners must use this same immutable policy and reject growth beyond it.
"""
from __future__ import annotations

from dataclasses import asdict, dataclass, fields, is_dataclass
import hashlib
import json
import re

NS = 1_000_000_000
MIB = 1024 * 1024
MAX_U64 = (1 << 64) - 1
MAX_I64 = (1 << 63) - 1
PAIR_COUNT = 5
RUN_COUNT = 10
MIN_SAMPLES = 22
MAX_SAMPLES = 100_000
MAX_STATUS_BODY_BYTES = MIB
MAX_METRICS_BODY_BYTES = 16 * MIB
CAPTURE_MANIFEST_BYTES = MIB
MAX_WIRE_BYTES = 64 * MIB
MAX_CONTROL_FILES = 256
MAX_FILE_BYTES = 256 * MIB
MAX_TOTAL_BYTES = 2 * 1024 * MIB
_VARIANTS = ("one_lane", "four_lane")
_LABEL = re.compile(r"[a-z][a-z0-9_.-]{0,127}")


class BudgetError(ValueError):
    """A static admission failure code without runtime configuration or secrets."""


def _require(condition: bool, code: str) -> None:
    if not condition:
        raise BudgetError(code)


def _integer(value: int, minimum: int = 0, maximum: int = MAX_U64) -> int:
    _require(type(value) is int and minimum <= value <= maximum, "integer_outside_bounds")
    return value


def _add(left: int, right: int) -> int:
    _integer(left)
    _integer(right)
    _require(right <= MAX_U64 - left, "arithmetic_overflow")
    return left + right


def _multiply(left: int, right: int) -> int:
    _integer(left)
    _integer(right)
    _require(left == 0 or right <= MAX_U64 // left, "arithmetic_overflow")
    return left * right


def _sum(values: tuple[int, ...]) -> int:
    result = 0
    for value in values:
        result = _add(result, value)
    return result


def _tuple(value: tuple, element_type: type, maximum: int) -> None:
    _require(type(value) is tuple and len(value) <= maximum, "bounded_tuple_required")
    _require(all(type(item) is element_type for item in value), "item_type_invalid")


def _label(value: str) -> None:
    # Ledger labels are bounded identities, not paths or filesystem authority.
    _require(type(value) is str and _LABEL.fullmatch(value) is not None, "label_invalid")


def _bytes_per_capture(policy: CapturePolicy, peers: int) -> int:
    body_bytes = min(MAX_WIRE_BYTES, _multiply(peers,
                    _add(policy.status_body_bytes, policy.metrics_body_bytes)))
    return _add(CAPTURE_MANIFEST_BYTES, body_bytes)


@dataclass(frozen=True, slots=True)
class CapturePolicy:
    """Immutable per-peer body limits; apply to preflight and every later sample.

    These bounds do not change node query limits, select metrics, or permit
    truncation. Manifest and complete-wire ceilings remain fixed by the protocol.
    """

    status_body_bytes: int = 128 * 1024
    metrics_body_bytes: int = MIB

    def __post_init__(self) -> None:
        _integer(self.status_body_bytes, 1, MAX_STATUS_BODY_BYTES)
        _integer(self.metrics_body_bytes, 1, MAX_METRICS_BODY_BYTES)


@dataclass(frozen=True, slots=True)
class CaptureGeometry:
    """Trusted fixed peer count and capture schedule, in integer nanoseconds.

    Warmup has one preflight capture. Sampling covers measurement and its complete
    positive drain, including both endpoints. Full workload/latency/Clock timeout
    validation remains with its existing owners; this is the resource geometry.
    """

    peers: int
    interval_ns: int
    measurement_ns: int
    drain_ns: int

    def __post_init__(self) -> None:
        _integer(self.peers, 4, 64)
        _integer(self.interval_ns, 2_000_000, 60 * NS)
        _integer(self.measurement_ns, 1, MAX_I64)
        _integer(self.drain_ns, 1, 300 * NS)
        _require(self.interval_ns % 1_000_000 == 0, "cadence_not_milliseconds")
        _require(self.measurement_ns % self.interval_ns == 0
                 and self.drain_ns % self.interval_ns == 0, "geometry_not_divisible")
        _require(self.measurement_ns >= _multiply(20, self.interval_ns),
                 "measurement_intervals_below_minimum")
        _require(self.final_offset_ns <= MAX_I64, "timing_overflow")
        _require(MIN_SAMPLES <= self.sample_count <= MAX_SAMPLES, "sample_count_outside_bounds")

    @property
    def final_offset_ns(self) -> int:
        """Scheduled start of the final resource sample, without timeout padding."""
        return _add(self.measurement_ns, self.drain_ns)

    @property
    def sample_count(self) -> int:
        """Inclusive scheduled samples; excludes preflight and manifest-free finish."""
        return _add(self.final_offset_ns // self.interval_ns, 1)

    @property
    def captures_per_run(self) -> int:
        """Every sample plus the single preflight capture."""
        return _add(self.sample_count, 1)


@dataclass(frozen=True, slots=True)
class StaticFile:
    """Actual size of one independently pinned control file, including empty files.

    Its label must be unique in the complete reservation ledger. The caller owns
    the size/hash/path authentication and one-to-one physical file mapping.
    """

    label: str
    size_bytes: int

    def __post_init__(self) -> None:
        _label(self.label)
        _integer(self.size_bytes, 0, MAX_FILE_BYTES)


@dataclass(frozen=True, slots=True)
class FileBudget:
    """Positive maximum allocation for one dynamic file, not an observed size."""

    label: str
    max_bytes: int

    def __post_init__(self) -> None:
        _label(self.label)
        _integer(self.max_bytes, 1, MAX_FILE_BYTES)


@dataclass(frozen=True, slots=True)
class RunBudget:
    """One mandatory sampled run with all dynamic artifact allocations explicit.

    Additional support files must be individually listed, never hidden in a
    multi-file allocation. Empty support is an explicit declaration of none.
    """

    pair_index: int
    variant: str
    geometry: CaptureGeometry
    collector_journal: FileBudget
    transaction_trace: FileBudget
    canonical_proof: FileBudget
    trial_log: FileBudget
    raw_run: FileBudget
    support: tuple[FileBudget, ...]

    def __post_init__(self) -> None:
        _integer(self.pair_index, 1, PAIR_COUNT)
        _require(type(self.variant) is str and self.variant in _VARIANTS, "variant_invalid")
        _require(type(self.geometry) is CaptureGeometry, "geometry_type_invalid")
        for item in (self.collector_journal, self.transaction_trace, self.canonical_proof,
                     self.trial_log, self.raw_run):
            _require(type(item) is FileBudget, "artifact_budget_required")
        _tuple(self.support, FileBudget, MAX_CONTROL_FILES)

    @property
    def files(self) -> tuple[FileBudget, ...]:
        """Complete individually bounded dynamic control files for this run."""
        return (self.collector_journal, self.transaction_trace, self.canonical_proof,
                self.trial_log, self.raw_run, *self.support)


@dataclass(frozen=True, slots=True)
class EvidenceBudget:
    """Admitted immutable ledger and its worst-case byte/member reservations.

    Raw resource members use a separate exact count, not the control-file limit.
    This module does not implement that typed scanner or writer enforcement.
    """

    policy: CapturePolicy
    geometry: CaptureGeometry
    runs: tuple[RunBudget, ...]
    static_files: tuple[StaticFile, ...]
    control_budgets: tuple[FileBudget, ...]
    members_per_capture: int
    members_per_run: int
    resource_member_count: int
    resource_capture_count: int
    bytes_per_capture: int
    resource_bytes_per_run: int
    resource_bytes: int
    control_file_count: int
    static_bytes: int
    dynamic_bytes: int
    total_bytes: int

    @property
    def remaining_bytes(self) -> int:
        """Unallocated global capacity, never an implicit per-writer allocation."""
        return MAX_TOTAL_BYTES - self.total_bytes


def admit_experiment(*, policy: CapturePolicy, runs: tuple[RunBudget, ...],
                     static_files: tuple[StaticFile, ...], manifest: FileBudget,
                     report: FileBudget, other_control: tuple[FileBudget, ...]) -> EvidenceBudget:
    """Reject impossible experiments using only bounded immutable input values.

    Requires all five pairs and both variants exactly once, with identical P/I/T/D
    and one shared capture policy. Counts every raw body/manifest and every static
    or dynamic control file under the unchanged 2 GiB total/256 MiB file ceilings.
    No preflight-size extrapolation, compression discount or unsampled form exists.
    """
    _require(type(policy) is CapturePolicy, "capture_policy_required")
    _tuple(runs, RunBudget, RUN_COUNT)
    _require(len(runs) == RUN_COUNT, "exact_ten_runs_required")
    _require({(run.pair_index, run.variant) for run in runs}
             == {(pair, variant) for pair in range(1, PAIR_COUNT + 1) for variant in _VARIANTS},
             "pair_variant_coverage_invalid")
    geometry = runs[0].geometry
    _require(all(run.geometry == geometry for run in runs), "run_geometry_mismatch")
    _tuple(static_files, StaticFile, MAX_CONTROL_FILES)
    _require(bool(static_files), "pinned_static_files_required")
    _require(type(manifest) is FileBudget and type(report) is FileBudget, "control_budget_required")
    _tuple(other_control, FileBudget, MAX_CONTROL_FILES)
    control_budgets = (manifest, report, *other_control)
    dynamic_files = (*control_budgets, *(item for run in runs for item in run.files))
    control_count = _add(len(static_files), len(dynamic_files))
    _require(control_count <= MAX_CONTROL_FILES, "control_file_count_exceeded")
    labels = tuple(item.label for item in (*static_files, *dynamic_files))
    _require(len(set(labels)) == control_count, "duplicate_file_allocation")

    members = _add(_multiply(2, geometry.peers), 1)
    members_per_run = _multiply(geometry.captures_per_run, members)
    per_capture = _bytes_per_capture(policy, geometry.peers)
    per_run = _multiply(geometry.captures_per_run, per_capture)
    resource_bytes = _multiply(RUN_COUNT, per_run)
    static_bytes = _sum(tuple(item.size_bytes for item in static_files))
    dynamic_bytes = _sum(tuple(item.max_bytes for item in dynamic_files))
    total_bytes = _sum((resource_bytes, static_bytes, dynamic_bytes))
    _require(total_bytes <= MAX_TOTAL_BYTES, "global_byte_reservation_exceeded")
    return EvidenceBudget(
        policy, geometry, runs, static_files, control_budgets, members, members_per_run,
        _multiply(RUN_COUNT, members_per_run), _multiply(RUN_COUNT, geometry.captures_per_run),
        per_capture, per_run, resource_bytes, control_count, static_bytes, dynamic_bytes, total_bytes,
    )


def _same_owned(actual, expected) -> bool:
    if type(actual) is not type(expected):
        return False
    if type(expected) is tuple:
        return len(actual) == len(expected) and all(
            _same_owned(left, right) for left, right in zip(actual, expected, strict=True))
    if is_dataclass(expected):
        return all(_same_owned(getattr(actual, item.name), getattr(expected, item.name))
                   for item in fields(expected))
    return actual == expected


def _readmit(experiment: EvidenceBudget) -> EvidenceBudget:
    _require(type(experiment) is EvidenceBudget, "admitted_experiment_required")
    _tuple(experiment.control_budgets, FileBudget, MAX_CONTROL_FILES)
    _require(len(experiment.control_budgets) >= 2, "control_budget_required")
    # Re-run constructor validation too: EvidenceBudget is a freely constructible
    # dataclass, not a capability that authenticates its numerical summaries.
    _require(type(experiment.policy) is CapturePolicy, "capture_policy_required")
    experiment.policy.__post_init__()
    _tuple(experiment.runs, RunBudget, RUN_COUNT)
    _tuple(experiment.static_files, StaticFile, MAX_CONTROL_FILES)
    for run in experiment.runs:
        run.__post_init__()
        run.geometry.__post_init__()
        for item in run.files:
            item.__post_init__()
    for item in (*experiment.static_files, *experiment.control_budgets):
        item.__post_init__()
    result = admit_experiment(policy=experiment.policy, runs=experiment.runs,
        static_files=experiment.static_files, manifest=experiment.control_budgets[0],
        report=experiment.control_budgets[1], other_control=experiment.control_budgets[2:])
    _require(_same_owned(experiment, result), "admitted_experiment_mismatch")
    return result


@dataclass(frozen=True, slots=True)
class PerRunResourceBudget:
    """One exact run selected from a fully re-admitted immutable experiment.

    Consumers must call validate_run_budget, not treat dataclass construction as
    authority. The launcher still independently authenticates the experiment's
    public inputs; a self-contained document cannot establish that identity.
    """

    experiment: EvidenceBudget
    run: RunBudget

    @property
    def policy(self) -> CapturePolicy:
        """Single immutable body policy for all ten runs."""
        return self.experiment.policy

    @property
    def geometry(self) -> CaptureGeometry:
        """Exact selected peer count and resource cadence."""
        return self.run.geometry

    @property
    def capture_count(self) -> int:
        """Preflight plus all inclusive samples."""
        return self.geometry.captures_per_run

    @property
    def members_per_capture(self) -> int:
        """Exact two bodies per peer and one manifest."""
        return self.experiment.members_per_capture

    @property
    def member_count(self) -> int:
        """Exact total raw files for this run."""
        return self.experiment.members_per_run

    @property
    def bytes_per_capture(self) -> int:
        """Conservative complete capture allocation, including its manifest."""
        return self.experiment.bytes_per_capture

    @property
    def resource_bytes(self) -> int:
        """Maximum raw body and manifest bytes for this run."""
        return self.experiment.resource_bytes_per_run

    @property
    def journal(self) -> FileBudget:
        """The one declared journal allocation, shared by all journal consumers."""
        return self.run.collector_journal


def select_run_budget(experiment: EvidenceBudget, pair_index: int, variant: str) -> PerRunResourceBudget:
    """Re-admit the complete ledger before projecting a selected run's limits."""
    _integer(pair_index, 1, PAIR_COUNT)
    _require(type(variant) is str and variant in _VARIANTS, "variant_invalid")
    admitted = _readmit(experiment)
    run = next(item for item in admitted.runs if (item.pair_index, item.variant) == (pair_index, variant))
    return PerRunResourceBudget(admitted, run)


def validate_run_budget(allocation: PerRunResourceBudget) -> PerRunResourceBudget:
    """Reject forged summaries, substituted runs and unadmitted projections."""
    _require(type(allocation) is PerRunResourceBudget and type(allocation.run) is RunBudget,
             "per_run_budget_required")
    result = select_run_budget(allocation.experiment, allocation.run.pair_index, allocation.run.variant)
    _require(_same_owned(allocation, result), "per_run_budget_mismatch")
    return result


def _object(value, names):
    _require(type(value) is dict and set(value) == set(names), "budget_object_fields_invalid")
    return value


def _array(value, maximum):
    _require(type(value) is list and len(value) <= maximum, "budget_array_invalid")
    return value


def _file_input(value):
    item = _object(value, ("label", "max_bytes"))
    return FileBudget(item["label"], item["max_bytes"])


def parse_run_budget(value: dict) -> PerRunResourceBudget:
    """Parse exact public admission inputs plus selected pair/variant, without I/O.

    The enclosing runtime config owns its existing 1 MiB framing/duplicate-key
    rejection. Every object/list here has one mandatory shape with no old form or
    computed-budget fields. The caller must independently pin these public inputs.
    """
    root = _object(value, ("experiment", "pair_index", "variant"))
    experiment = _object(root["experiment"], ("capture_policy", "runs", "static_files",
                                             "manifest", "report", "other_control"))
    policy = CapturePolicy(**_object(experiment["capture_policy"],
                                    ("status_body_bytes", "metrics_body_bytes")))
    runs = []
    for row in _array(experiment["runs"], RUN_COUNT):
        _object(row, ("pair_index", "variant", "geometry", "collector_journal", "transaction_trace",
                      "canonical_proof", "trial_log", "raw_run", "support"))
        geometry = CaptureGeometry(**_object(row["geometry"],
                                             ("peers", "interval_ns", "measurement_ns", "drain_ns")))
        runs.append(RunBudget(row["pair_index"], row["variant"], geometry,
            *(_file_input(row[name]) for name in ("collector_journal", "transaction_trace", "canonical_proof",
                                                 "trial_log", "raw_run")),
            tuple(_file_input(item) for item in _array(row["support"], MAX_CONTROL_FILES))))
    static = tuple(StaticFile(**_object(item, ("label", "size_bytes")))
                   for item in _array(experiment["static_files"], MAX_CONTROL_FILES))
    admitted = admit_experiment(policy=policy, runs=tuple(runs), static_files=static,
        manifest=_file_input(experiment["manifest"]), report=_file_input(experiment["report"]),
        other_control=tuple(_file_input(item) for item in _array(experiment["other_control"], MAX_CONTROL_FILES)))
    return select_run_budget(admitted, root["pair_index"], root["variant"])


def run_budget_inputs(allocation: PerRunResourceBudget) -> dict:
    """Return canonical public input fields for the strict runtime-config decoder.

    This is an explicit input object, not a signature, digest, reduction or proof
    of independently expected experiment identity. No secrets are included.
    """
    selected = validate_run_budget(allocation)
    experiment = selected.experiment
    return {"experiment": {"capture_policy": asdict(selected.policy),
            "runs": [asdict(run) | {"support": [asdict(item) for item in run.support]}
                     for run in experiment.runs],
            "static_files": [asdict(item) for item in experiment.static_files],
            "manifest": asdict(experiment.control_budgets[0]),
            "report": asdict(experiment.control_budgets[1]),
            "other_control": [asdict(item) for item in experiment.control_budgets[2:]]},
            "pair_index": selected.run.pair_index, "variant": selected.run.variant}


def canonical_run_budget_bytes(allocation: PerRunResourceBudget) -> bytes:
    """Re-admit and canonically encode only the complete public run-budget inputs.

    The shared launcher/worker identity is ASCII JSON with sorted object keys,
    ensure_ascii=True, separators=(',', ':'), allow_nan=False, and no trailing
    newline. Arrays retain their declared order. Runtime configuration, endpoint
    and credential objects are never inputs to this function.
    """
    value = run_budget_inputs(allocation)
    return json.dumps(value, sort_keys=True, ensure_ascii=True,
                      separators=(',', ':'), allow_nan=False).encode('ascii')


def run_budget_sha256(allocation: PerRunResourceBudget) -> str:
    """Hash the one canonical public encoding, after complete re-admission."""
    return hashlib.sha256(canonical_run_budget_bytes(allocation)).hexdigest()
