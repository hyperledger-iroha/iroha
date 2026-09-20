"""Decode the sole fixed experiment schema and complete budget inputs.

This pure prelaunch boundary creates no files or processes. Decoding a plan
checks its wire shape; ``decode_fixed_inputs`` additionally admits every native
policy and every run allocation through the existing plan and budget owners.
There are no omitted-policy defaults, compatibility schemas or budget decoders.
"""
from __future__ import annotations

from dataclasses import fields
import json

from resource_evidence_budget import (
    EvidenceBudget, canonical_run_budget_bytes, parse_run_budget,
)
from scaling_experiment_plan import (
    ExperimentPlan, ResourceLimits, MAX_PLAN_BYTES, PLAN_SCHEMA,
    admit_plan, plan_bytes,
)
from scaling_fixed_trial import (
    TrialPlan, GeneratorPlan, NativeLoadPlan, ReaderBudget, CollectionLimits,
    FactsBudget, NativeOutputBudget,
)

MAX_CONFIG_BYTES = MAX_PLAN_BYTES
MAX_JSON_DEPTH = 5
MAX_JSON_TOKENS = 8192
MAX_JSON_NODES = 4096
MAX_STRING_CHARS = 4096
_POLICIES = (
    ('generator', GeneratorPlan), ('load', NativeLoadPlan),
    ('reader', ReaderBudget), ('collection', CollectionLimits),
    ('facts', FactsBudget), ('native_outputs', NativeOutputBudget),
)


class ExperimentConfigError(ValueError):
    """Closed invalid-input code without configuration contents or paths."""


def _require(condition):
    if not condition:
        raise ExperimentConfigError('fixed_experiment_config_invalid')


def _framing(raw):
    """Bound tokens, string wire size and nesting before JSON allocates a tree."""
    _require(type(raw) is bytes and 1 < len(raw) <= MAX_CONFIG_BYTES and raw.isascii())
    stack, quoted, escaped, tokens, string_bytes = [], False, False, 0, 0
    for byte in raw:
        if quoted:
            string_bytes += 1
            # An escaped supplementary character occupies two six-byte UTF-16
            # escapes in the canonical ASCII form. Bound before JSON unescaping.
            _require(string_bytes <= 12 * MAX_STRING_CHARS + 1)
            if escaped:
                escaped = False
            elif byte == 92:
                escaped = True
            elif byte == 34:
                quoted = False
        elif byte == 34:
            quoted, string_bytes = True, 0
            tokens += 1
        elif byte in (91, 123):
            stack.append(93 if byte == 91 else 125)
            _require(len(stack) <= MAX_JSON_DEPTH)
            tokens += 1
        elif byte in (93, 125):
            _require(bool(stack) and stack.pop() == byte)
            tokens += 1
        elif byte in (44, 58):
            tokens += 1
        _require(tokens <= MAX_JSON_TOKENS)
    _require(not stack and not quoted and not escaped)


def _pairs(rows):
    _require(len(rows) <= 32)
    result = {}
    for key, value in rows:
        _require(type(key) is str and 0 < len(key) <= 128 and key not in result)
        result[key] = value
    return result


def _integer(token):
    _require(len(token) <= 40)
    value = int(token)
    _require(-(1 << 127) < value < (1 << 128))
    return value


def _reject_number(_):
    raise ExperimentConfigError('fixed_experiment_config_invalid')


def _json(raw):
    _framing(raw)
    value = json.loads(raw.decode('ascii'), object_pairs_hook=_pairs,
                       parse_int=_integer, parse_float=_reject_number,
                       parse_constant=_reject_number)
    pending, nodes = [value], 0
    while pending:
        item = pending.pop()
        nodes += 1
        _require(nodes <= MAX_JSON_NODES)
        if type(item) is dict:
            pending.extend(item.values())
        elif type(item) is list:
            _require(len(item) <= 256)
            pending.extend(item)
        elif type(item) is str:
            _require(0 < len(item) <= MAX_STRING_CHARS)
        else:
            _require(type(item) is int)
    return value


def _object(value, names):
    _require(type(value) is dict and value.keys() == set(names))
    return value


def _scalar(value, expected):
    _require(type(value) is expected)
    return value


def _record_values(value, record):
    """Validate exact owner-declared scalar fields before constructing records."""
    schema = fields(record)
    _object(value, (field.name for field in schema))
    result = {}
    for field in schema:
        # Policy annotations are trusted source metadata, never evaluated input.
        _require(field.type in (str, int, 'str', 'int'))
        expected = str if field.type in (str, 'str') else int
        result[field.name] = _scalar(value[field.name], expected)
    return result


def decode_plan(raw: bytes) -> ExperimentPlan:
    """Decode the ``plan_bytes`` schema; full admission also needs its budget.

    Every object, scalar and all ten trial shapes are checked before any policy
    record is constructed. JSON whitespace and object-key order are immaterial;
    public bytes use ``plan_bytes``. Call ``decode_fixed_inputs`` before execution.
    """
    try:
        value = _json(raw)
        _object(value, ('schema', *(field.name for field in fields(ExperimentPlan))))
        _require(type(value['schema']) is str and value['schema'] == PLAN_SCHEMA)
        namespace = _scalar(value['seed_namespace'], str)
        trial_timeout = _scalar(value['trial_timeout_ns'], int)
        experiment_timeout = _scalar(value['experiment_timeout_ns'], int)
        limits = _record_values(value['resource_limits'], ResourceLimits)
        rows = value['trials']
        _require(type(rows) is list and len(rows) == 10)
        trials = []
        for row in rows:
            _object(row, (field.name for field in fields(TrialPlan)))
            policies = tuple(_record_values(row[name], record) for name, record in _POLICIES)
            reply = _scalar(row['replay_reply_max_bytes'], int)
            stop = _scalar(row['stop_timeout_ns'], int)
            trials.append((policies, reply, stop))
        owned = tuple(TrialPlan(
            *(record(**policy) for (_, record), policy in zip(_POLICIES, policies, strict=True)),
            reply, stop) for policies, reply, stop in trials)
        result = ExperimentPlan(namespace, owned, trial_timeout, experiment_timeout,
                                ResourceLimits(**limits))
        _require(json.loads(plan_bytes(result)) == value)
        return result
    except (ValueError, TypeError, OverflowError, RecursionError):
        raise ExperimentConfigError('fixed_experiment_config_invalid') from None


def decode_fixed_inputs(plan_raw: bytes, budget_raw: bytes) -> tuple[ExperimentPlan, EvidenceBudget, bytes]:
    """Own and admit the fixed plan plus the existing complete budget format.

    The budget envelope selects pair 1 / one_lane and still contains every run.
    Its schema and all allocation arithmetic belong exclusively to
    ``parse_run_budget``. Equivalent JSON values have the same canonical public
    bytes; strings and array order are preserved. The plan's static size uses
    those canonical bytes, independently of the input file's formatting.
    """
    try:
        plan = decode_plan(plan_raw)
        value = _json(budget_raw)
        selected = parse_run_budget(value)
        _require(selected.run.pair_index == 1 and selected.run.variant == 'one_lane')
        _require(json.loads(canonical_run_budget_bytes(selected)) == value)
        owned_plan, owned_budget, canonical = admit_plan(plan, selected.experiment)
        _require(canonical == plan_bytes(plan))
        return owned_plan, owned_budget, canonical
    except (ValueError, TypeError, OverflowError, RecursionError):
        raise ExperimentConfigError('fixed_experiment_config_invalid') from None
