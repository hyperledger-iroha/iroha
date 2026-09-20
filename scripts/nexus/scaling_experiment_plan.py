"""One immutable first-release ten-trial plan and its public input projection.

Admission is pure: all trial policies and allocations are checked before an
experiment creates any filesystem or process state. Native execution and host
runtime qualification remain with their original owners.
"""
from __future__ import annotations

from dataclasses import asdict, dataclass, fields, replace
from fractions import Fraction
import hashlib
import json
import re

from resource_evidence_budget import (EvidenceBudget, RUN_FILE_FIELDS,
    select_run_budget, parse_run_budget, run_budget_inputs)
from scaling_command import MAX_TRIAL_NS
from scaling_fixed_trial import (TrialPlan, GeneratorPlan, NativeLoadPlan,
    ReaderBudget, CollectionLimits, FactsBudget, NativeOutputBudget, fingerprint)
from scaling_native_facts_inputs import reader_snapshot, budget_snapshot
from scaling_native_outputs import _budget as output_snapshot
from scaling_vector_collection import _limits as collection_snapshot

RUN_KEYS = tuple((pair, variant) for pair in range(1, 6) for variant in ('one_lane', 'four_lane'))
MAX_PLAN_BYTES = 1024 * 1024
PLAN_SCHEMA = 'iroha.sumeragi_v2.multilane_scaling.fixed_plan.v1'
_RECORDS = (('generator', GeneratorPlan), ('load', NativeLoadPlan),
    ('reader', ReaderBudget), ('collection', CollectionLimits),
    ('facts', FactsBudget), ('native_outputs', NativeOutputBudget))


class ExperimentPlanError(ValueError):
    """Closed invalid-plan code without runtime configuration material."""


def require(condition):
    if not condition: raise ExperimentPlanError('fixed_experiment_plan_invalid')


def encode(value, maximum=MAX_PLAN_BYTES):
    """Encode a bounded public projection, rejecting non-finite JSON values."""
    # The callers build fixed bounded objects; iterencode additionally enforces
    # the byte reservation before retaining a complete potentially large result.
    encoder = json.JSONEncoder(sort_keys=True, ensure_ascii=True, allow_nan=False, separators=(',', ':'))
    parts, size = [], 0
    for part in encoder.iterencode(value):
        raw = part.encode('ascii'); size += len(raw)
        require(size <= maximum)
        parts.append(raw)
    return b''.join(parts)


@dataclass(frozen=True, slots=True)
class ResourceLimits:
    """Same acceptance ceilings for the observed whole peer cohort in ten runs.

    The report compares these ceilings with sampled queue/index measurements,
    aggregate peer RSS and declared storage roots. Sampling does not enforce
    continuous host quotas or establish unobserved peaks.
    """
    queue_depth_max: int
    index_entries_max: int
    memory_bytes_max: int
    disk_bytes_max: int
    min_latency_samples: int = 100

    def validate(self):
        for value in (self.queue_depth_max, self.index_entries_max, self.memory_bytes_max, self.disk_bytes_max):
            require(type(value) is int and 0 < value <= 1 << 53)
        require(type(self.min_latency_samples) is int and 100 <= self.min_latency_samples <= 65_536)


@dataclass(frozen=True, slots=True)
class ExperimentPlan:
    """Fixed pair order, workload, per-trial deadlines and original outer scope."""
    seed_namespace: str
    trials: tuple[TrialPlan, ...]
    trial_timeout_ns: int
    experiment_timeout_ns: int
    resource_limits: ResourceLimits


def plan_bytes(value: ExperimentPlan) -> bytes:
    """Canonical public plan; contains no development seed or runtime path."""
    require(type(value) is ExperimentPlan and type(value.trials) is tuple and len(value.trials) == 10)
    require(type(value.seed_namespace) is str and re.fullmatch(r'[A-Za-z0-9._-]{1,128}', value.seed_namespace))
    require(all(type(item) is TrialPlan for item in value.trials))
    require(type(value.trial_timeout_ns) is int and 0 < value.trial_timeout_ns <= MAX_TRIAL_NS)
    require(type(value.experiment_timeout_ns) is int
            and 10 * value.trial_timeout_ns < value.experiment_timeout_ns <= 12 * 60 * 60 * 1_000_000_000)
    require(type(value.resource_limits) is ResourceLimits); value.resource_limits.validate()
    # Reject foreign containers and oversized scalars before asdict can recurse
    # or invoke deepcopy on caller objects. The admitted plan is a fixed tree of
    # exact native policy records, each containing only bounded scalar fields.
    for trial in value.trials:
        for name, expected in _RECORDS:
            record = getattr(trial, name)
            require(type(record) is expected)
            for field in fields(expected):
                item = getattr(record, field.name)
                require((type(item) is int and -(1 << 127) < item < (1 << 128))
                        or (type(item) is str and 0 < len(item) <= 4096))
        require(type(trial.stop_timeout_ns) is int and 0 < trial.stop_timeout_ns <= 300_000_000_000)
        require(type(trial.replay_reply_max_bytes) is int
                and 0 < trial.replay_reply_max_bytes <= 256 * 1024 * 1024)
    return encode({'schema': PLAN_SCHEMA, **asdict(value)})


def admit_plan(value: ExperimentPlan, budget: EvidenceBudget) -> tuple[ExperimentPlan, EvidenceBudget, bytes]:
    """Check every native policy and the exact equal-load pairing, then own it."""
    raw = plan_bytes(value)
    require(type(budget) is EvidenceBudget)
    selected = parse_run_budget(run_budget_inputs(select_run_budget(budget, 1, 'one_lane')))
    owned_budget = selected.experiment
    require(tuple(item.label for item in owned_budget.static_files) == ('identity', 'plan', 'source_closure')
            and tuple(item.label for item in owned_budget.control_budgets) == ('manifest', 'report'))
    require(next(item.size_bytes for item in owned_budget.static_files if item.label == 'plan') == len(raw))
    first = None
    first_caps = None
    owned = []
    for expected, trial in zip(RUN_KEYS, value.trials, strict=True):
        pair, variant = expected
        allocation = select_run_budget(owned_budget, pair, variant)
        trial.generator.validate()
        require((trial.load.pair_index, trial.load.variant) == expected
                and trial.generator.lane_count == (1 if variant == 'one_lane' else 4))
        require(trial.load.seed == hashlib.sha256(f'{value.seed_namespace}:{pair}'.encode()).hexdigest())
        _, lifetime = trial.load.validate(trial.generator.account_count, allocation)
        require(lifetime < value.trial_timeout_ns)
        count = Fraction(trial.load.offered_load_tps) * trial.load.measurement_ns / 1_000_000_000
        require(-(-count.numerator // count.denominator) >= value.resource_limits.min_latency_samples)
        reader_snapshot(trial.reader); budget_snapshot(trial.facts)
        collection_snapshot(trial.collection); output_snapshot(trial.native_outputs)
        require(type(trial.stop_timeout_ns) is int and 0 < trial.stop_timeout_ns <= 300_000_000_000)
        require(type(trial.replay_reply_max_bytes) is int and 0 < trial.replay_reply_max_bytes <= 256 * 1024 * 1024)
        roles = ('finality', 'queries', 'facts', 'request', 'bundle', 'proof')
        names = ('native_finality', 'native_queries', 'native_facts', 'native_request', 'native_bundle', 'canonical_proof')
        caps = tuple(getattr(allocation.run, name).max_bytes for name in names)
        require(tuple(getattr(trial.native_outputs, role) for role in roles) == caps
                and trial.native_outputs.total == sum(caps))
        run_caps = tuple(getattr(allocation.run, name).max_bytes for name in RUN_FILE_FIELDS)
        if first_caps is None: first_caps = run_caps
        require(run_caps == first_caps)
        normalized = replace(trial, generator=replace(trial.generator, lane_count=1),
                             load=replace(trial.load, pair_index=1, variant='one_lane', seed='0' * 64))
        key = fingerprint(normalized)
        if first is None: first = key
        require(key == first)
        # Every field in these exact, validated dataclasses is a scalar primitive.
        # Reconstruct each nested record rather than sharing caller-owned objects.
        owned.append(TrialPlan(*(type(item)(**asdict(item)) for item in
            (trial.generator, trial.load, trial.reader, trial.collection, trial.facts, trial.native_outputs)),
            trial.replay_reply_max_bytes, trial.stop_timeout_ns))
    result = ExperimentPlan(value.seed_namespace, tuple(owned), value.trial_timeout_ns,
                            value.experiment_timeout_ns, ResourceLimits(**asdict(value.resource_limits)))
    require(plan_bytes(result) == raw)
    return result, owned_budget, raw
