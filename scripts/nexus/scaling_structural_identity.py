"""Bounded typed identity for repeatedly checking an already admitted policy.

Factories retain the existing complete admission and canonical encoding. Later
checks compare every exact record, tuple and primitive against a separately
owned structural pin, without rebuilding JSON. This is neither admission from
an arbitrary digest nor filesystem, runtime, native or release authority.
"""
from dataclasses import dataclass, fields

import resource_evidence_budget as budget

_BUDGET_RECORDS = (budget.CapturePolicy, budget.CaptureGeometry, budget.StaticFile,
    budget.FileBudget, budget.RunBudget, budget.EvidenceBudget, budget.PerRunResourceBudget)
_MAX_ENCODING = 1024 * 1024
_MAX_NODES = 8192


class StructuralIdentityError(ValueError):
    """Closed failure when the admitted typed policy or its encoding changes."""


def _require(value):
    if not value:
        raise StructuralIdentityError('structural_identity_changed')


def _capture(value, records, leaves, count, depth=0):
    count[0] += 1
    _require(depth <= 8 and count[0] <= _MAX_NODES)
    kind = type(value)
    if kind is int or kind is str:
        _require((kind is int and -(1 << 127) < value < 1 << 128)
                 or (kind is str and len(value) <= 4096))
        leaves.append(value)
        return (kind, ())
    if kind is tuple:
        _require(len(value) <= budget.MAX_CONTROL_FILES)
        return (kind, tuple(_capture(item, records, leaves, count, depth + 1) for item in value))
    # Check exact classes before reading fields. Foreign properties, equality,
    # copying and caller-supplied field metadata are never invoked.
    _require(any(kind is record for record in records))
    return (kind, tuple((field.name, _capture(getattr(value, field.name), records, leaves, count, depth + 1))
                        for field in fields(kind)))


def _observe(value, schema, leaves):
    kind, children = schema
    _require(type(value) is kind)
    if kind is int or kind is str:
        leaves.append(value)
    elif kind is tuple:
        _require(len(value) == len(children))
        for item, child in zip(value, children, strict=True):
            _observe(item, child, leaves)
    else:
        for name, child in children:
            _observe(getattr(value, name), child, leaves)


@dataclass(frozen=True, slots=True)
class StructuralIdentity:
    """Deep-owned tuple/primitive pin; only the three factories admit policies.

    No source dataclass instance is retained in the schema or values. Consumers
    keep this pin beside their original canonical bytes and use checked_bytes
    inside their existing failure/metadata/deadline guards.
    """
    _schema: tuple
    _values: tuple
    _canonical: bytes

    def checked_bytes(self, value, canonical: bytes) -> bytes:
        """Check the complete unchanged typed tree and return its original bytes."""
        observed = []
        _observe(value, self._schema, observed)
        # Validate the complete tree's types before any source value equality.
        # Built-in immutable scalar comparison invokes no caller-defined code.
        _require(tuple(observed) == self._values)
        _require(type(canonical) is bytes and canonical == self._canonical)
        return canonical


def _make(value, canonical, records, encode):
    _require(type(canonical) is bytes and 0 < len(canonical) <= _MAX_ENCODING)
    leaves = []
    schema = _capture(value, records, leaves, [0])
    # Preserve full semantic re-admission/encoding once at pin creation. This
    # also rejects forged derived summaries and substituted selected runs.
    _require(encode(value) == canonical)
    result = StructuralIdentity(schema, tuple(leaves), canonical)
    result.checked_bytes(value, canonical)
    return result


def pin_run_budget(value: budget.PerRunResourceBudget, canonical: bytes) -> StructuralIdentity:
    """Pin the complete experiment and its selected run after full admission."""
    _require(type(value) is budget.PerRunResourceBudget)
    return _make(value, canonical, _BUDGET_RECORDS, budget.canonical_run_budget_bytes)


def pin_experiment_budget(value: budget.EvidenceBudget, canonical: bytes) -> StructuralIdentity:
    """Pin every run, fixed file allocation, geometry and derived ledger total."""
    _require(type(value) is budget.EvidenceBudget)
    return _make(value, canonical, _BUDGET_RECORDS,
        lambda item: budget.canonical_run_budget_bytes(budget.select_run_budget(item, 1, 'one_lane')))


def pin_experiment_plan(value, canonical: bytes) -> StructuralIdentity:
    """Pin all ten exact trial policies while retaining canonical plan encoding."""
    # Deferred only to avoid TrialCaptures -> plan -> FixedTrial import cycles.
    # Construction happens after the normal owner modules have initialized.
    from scaling_experiment_plan import ExperimentPlan, ResourceLimits, _RECORDS, plan_bytes
    from scaling_fixed_trial import TrialPlan
    _require(type(value) is ExperimentPlan)
    records = (ExperimentPlan, ResourceLimits, TrialPlan, *(record for _, record in _RECORDS))
    return _make(value, canonical, records, plan_bytes)
