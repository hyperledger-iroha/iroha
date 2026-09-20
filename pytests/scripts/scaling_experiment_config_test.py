"""Strict pure CLI input decoding against the actual fixed ten-run fixture."""
import json
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/nexus'))
sys.path.insert(0, str(ROOT / 'scripts/tests'))
sys.path.insert(0, str(ROOT / 'pytests/scripts'))

import scaling_experiment_config as config
from scaling_experiment_custody_test import fixed_plan, budget_for
from scaling_experiment_plan import RUN_KEYS, plan_bytes
from resource_evidence_budget import canonical_run_budget_bytes, select_run_budget


def inputs():
    plan = fixed_plan()
    budget = budget_for(plan)
    return plan_bytes(plan), canonical_run_budget_bytes(select_run_budget(budget, 1, 'one_lane'))


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True).encode('ascii')


def at(value, path):
    for key in path:
        value = value[key]
    return value


def test_actual_ten_run_fixture_roundtrips_and_owns_every_policy_and_allocation():
    plan_raw, budget_raw = inputs()
    decoded = config.decode_plan(plan_raw)
    plan, budget, canonical = config.decode_fixed_inputs(plan_raw, budget_raw)
    assert canonical == plan_raw == plan_bytes(decoded) == plan_bytes(plan)
    assert tuple((trial.load.pair_index, trial.load.variant) for trial in plan.trials) == RUN_KEYS
    assert tuple((run.pair_index, run.variant) for run in budget.runs) == RUN_KEYS
    assert canonical_run_budget_bytes(select_run_budget(budget, 1, 'one_lane')) == budget_raw
    for left, right in zip(decoded.trials, plan.trials, strict=True):
        assert left is not right
        for name, record in config._POLICIES:
            assert type(getattr(left, name)) is type(getattr(right, name)) is record
            assert getattr(left, name) is not getattr(right, name)
            assert getattr(left, name) == getattr(right, name)
    again, other_budget, _ = config.decode_fixed_inputs(plan_raw, budget_raw)
    assert again is not plan and other_budget is not budget
    assert other_budget.runs[9] is not budget.runs[9]
    assert other_budget.runs[9].native_facts is not budget.runs[9].native_facts
    object.__setattr__(budget.runs[9].native_facts, 'max_bytes', 1)
    object.__setattr__(plan.trials[9].load, 'offered_load_tps', '1')
    assert other_budget.runs[9].native_facts.max_bytes != 1
    assert again.trials[9].load.offered_load_tps == '500'


def test_readable_whitespace_and_key_order_use_one_canonical_public_identity():
    plan_raw, budget_raw = inputs()
    def readable(raw):
        row = json.loads(raw)
        return ('\n \t' + json.dumps(dict(reversed(tuple(row.items()))), indent=2) + '\n').encode('ascii')
    plan, budget, canonical = config.decode_fixed_inputs(readable(plan_raw), readable(budget_raw))
    assert canonical == plan_raw == plan_bytes(plan)
    assert canonical_run_budget_bytes(select_run_budget(budget, 1, 'one_lane')) == budget_raw
    assert next(item.size_bytes for item in budget.static_files if item.label == 'plan') == len(canonical)


def test_exact_input_size_ceiling_is_accepted_before_rejecting_one_more_byte():
    plan_raw, budget_raw = inputs()
    padded_plan = plan_raw + b' ' * (config.MAX_CONFIG_BYTES - len(plan_raw))
    padded_budget = budget_raw + b' ' * (config.MAX_CONFIG_BYTES - len(budget_raw))
    _, _, canonical = config.decode_fixed_inputs(padded_plan, padded_budget)
    assert canonical == plan_raw
    for first, second in ((padded_plan + b' ', padded_budget), (padded_plan, padded_budget + b' ')):
        with pytest.raises(config.ExperimentConfigError):
            config.decode_fixed_inputs(first, second)


def test_json_string_escapes_are_values_and_escaped_duplicate_names_still_fail():
    plan_raw, budget_raw = inputs()
    escaped = plan_raw.replace(b'"fixed-test"', b'"\\u0066ixed-test"')
    _, _, canonical = config.decode_fixed_inputs(escaped, budget_raw)
    assert escaped != plan_raw and canonical == plan_raw
    duplicate = plan_raw[:-1] + b',"\\u0073eed_namespace":"fixed-test"}'
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(duplicate)


_OBJECT_PATHS = [(), ('resource_limits',), ('trials', 0),
                 *(('trials', 0, name) for name, _ in config._POLICIES)]


@pytest.mark.parametrize('path', _OBJECT_PATHS)
@pytest.mark.parametrize('change', ['unknown', 'missing'])
def test_every_plan_object_requires_all_and_only_its_declared_fields(path, change):
    plan_raw, _ = inputs()
    value = json.loads(plan_raw)
    row = at(value, path)
    if change == 'unknown':
        row['compatibility'] = 1
    else:
        row.pop(next(iter(row)))
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(encoded(value))


@pytest.mark.parametrize('path', _OBJECT_PATHS)
def test_duplicate_keys_are_rejected_at_every_plan_object_depth(path):
    plan_raw, _ = inputs()
    value = json.loads(plan_raw)
    row = at(value, path)
    key = next(iter(row))
    original = encoded(row)
    duplicate = original[:-1] + b',' + encoded({key: row[key]})[1:]
    raw = plan_raw.replace(original, duplicate, 1)
    assert raw != plan_raw
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(raw)


@pytest.mark.parametrize('value', [None, True, False, 1.25, 1e20, '600000000000', [], {}])
def test_trial_timeout_requires_an_exact_integer(value):
    plan_raw, _ = inputs()
    row = json.loads(plan_raw)
    row['trial_timeout_ns'] = value
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(encoded(row))


@pytest.mark.parametrize('name,record', config._POLICIES)
def test_every_native_policy_enforces_declared_integer_types(name, record):
    plan_raw, _ = inputs()
    value = json.loads(plan_raw)
    policy = value['trials'][9][name]
    key = next(key for key, item in policy.items() if type(item) is int)
    policy[key] = str(policy[key])
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(encoded(value))


@pytest.mark.parametrize('name,key', [('generator', 'chain_id'), ('generator', 'bind_host'),
    ('generator', 'public_host'), ('load', 'variant'), ('load', 'seed'), ('load', 'offered_load_tps')])
def test_declared_native_strings_never_accept_numbers(name, key):
    plan_raw, _ = inputs()
    value = json.loads(plan_raw)
    value['trials'][9][name][key] = 123
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(encoded(value))


@pytest.mark.parametrize('replacement', [b'1e3', b'NaN', b'Infinity', b'-Infinity', b'1' * 10000,
                                       str(1 << 128).encode(), str(-(1 << 127)).encode()])
def test_invalid_numbers_are_rejected_without_large_integer_conversion(replacement):
    plan_raw, _ = inputs()
    raw = plan_raw.replace(b'"trial_timeout_ns":600000000000', b'"trial_timeout_ns":' + replacement)
    assert raw != plan_raw
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(raw)


@pytest.mark.parametrize('schema', [None, 1, 'v1', 'iroha.sumeragi_v2.multilane_scaling.fixed_plan.v2'])
def test_no_alternate_or_implicit_schema(schema):
    plan_raw, _ = inputs()
    value = json.loads(plan_raw)
    value['schema'] = schema
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(encoded(value))


@pytest.mark.parametrize('count', [0, 9, 11])
def test_plan_has_exactly_ten_explicit_trials(count):
    plan_raw, _ = inputs()
    value = json.loads(plan_raw)
    value['trials'] = (value['trials'] * 2)[:count]
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(encoded(value))


def test_missing_default_or_string_number_never_invokes_a_policy_constructor(monkeypatch):
    plan_raw, _ = inputs()
    value = json.loads(plan_raw)
    value['trials'][9]['load']['resource_interval_ms'] = '10'
    def forbidden(*args, **kwargs):
        raise AssertionError('all trial shapes must be checked before construction')
    monkeypatch.setattr(config.GeneratorPlan, '__init__', forbidden)
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(encoded(value))
    value = json.loads(plan_raw)
    value['resource_limits'].pop('min_latency_samples')
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(encoded(value))


@pytest.mark.parametrize('raw', [b'[' * 20 + b'0' + b']' * 20,
    b'{"x":' + b'[' * 6 + b'0' + b']' * 6 + b'}', b'{"x":[]}',
    b'{"x":[' + b'0,' * 8192 + b'0]}', b'{"x":"' + b'a' * (12 * 4096 + 1) + b'"}',
    b'{"x":[}', b'{"x":"unterminated}', b'{}' + b' ' * config.MAX_CONFIG_BYTES])
def test_preparse_framing_rejects_depth_tokens_strings_mismatches_and_size(raw, monkeypatch):
    # The small well-framed row is intentionally a control for the parser hook.
    seen = []
    def parser(*args, **kwargs):
        seen.append(True)
        raise ValueError('controlled parser')
    monkeypatch.setattr(config.json, 'loads', parser)
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(raw)
    assert bool(seen) is (raw == b'{"x":[]}')


@pytest.mark.parametrize('kind', ['string', 'bytes_subclass', 'bytearray', 'memoryview', 'none'])
def test_only_exact_immutable_bytes_are_admitted_without_foreign_hooks(kind):
    class Foreign(bytes):
        def __len__(self):
            raise AssertionError('foreign hook')
        def decode(self, *args, **kwargs):
            raise AssertionError('foreign hook')
    values = {'string': '{}', 'bytes_subclass': Foreign(b'{}'), 'bytearray': bytearray(b'{}'),
              'memoryview': memoryview(b'{}'), 'none': None}
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(values[kind])


@pytest.mark.parametrize('case', ['order', 'seed', 'lane_count', 'workload', 'samples', 'reader',
    'facts', 'collection', 'native_outputs', 'lifetime', 'static_size'])
def test_full_inputs_use_actual_native_and_equal_work_admission(case):
    plan_raw, budget_raw = inputs()
    plan, budget = json.loads(plan_raw), json.loads(budget_raw)
    if case == 'order':
        plan['trials'][0], plan['trials'][1] = plan['trials'][1], plan['trials'][0]
    elif case == 'seed':
        plan['trials'][9]['load']['seed'] = 'f' * 64
    elif case == 'lane_count':
        plan['trials'][9]['generator']['lane_count'] = 1
    elif case == 'workload':
        plan['trials'][9]['load']['offered_load_tps'] = '1000'
    elif case == 'samples':
        for trial in plan['trials']:
            trial['load']['offered_load_tps'] = '40'
    elif case in ('reader', 'facts', 'collection', 'native_outputs'):
        policy = plan['trials'][9][case]
        policy[next(iter(policy))] = 0
    elif case == 'lifetime':
        plan['trial_timeout_ns'] = 1
    elif case == 'static_size':
        budget['experiment']['static_files'][1]['size_bytes'] += 1
    if case != 'static_size':
        budget['experiment']['static_files'][1]['size_bytes'] = len(encoded(plan))
    with pytest.raises(config.ExperimentConfigError):
        config.decode_fixed_inputs(encoded(plan), encoded(budget))


@pytest.mark.parametrize('case', ['selected_pair', 'selected_variant', 'selected_bool', 'missing_run',
    'duplicate_run', 'last_run_cap', 'geometry', 'unknown', 'version', 'computed', 'retired_role',
    'other_control', 'duplicate_label', 'global_size', 'number', 'missing_static'])
def test_budget_is_the_existing_complete_exact_first_run_envelope(case):
    plan_raw, budget_raw = inputs()
    value = json.loads(budget_raw)
    experiment = value['experiment']
    if case == 'selected_pair':
        value['pair_index'] = 2
    elif case == 'selected_variant':
        value['variant'] = 'four_lane'
    elif case == 'selected_bool':
        value['pair_index'] = True
    elif case == 'missing_run':
        experiment['runs'].pop()
    elif case == 'duplicate_run':
        experiment['runs'][9] = experiment['runs'][0]
    elif case == 'last_run_cap':
        experiment['runs'][9]['raw_run']['max_bytes'] += 1
    elif case == 'geometry':
        experiment['runs'][9]['geometry']['peers'] = 5
    elif case == 'unknown':
        value['unknown'] = 1
    elif case == 'version':
        value['schema'] = 'v1'
    elif case == 'computed':
        experiment['total_bytes'] = 1
    elif case == 'retired_role':
        experiment['runs'][0]['trial_log'] = experiment['runs'][0].pop('raw_run')
    elif case == 'other_control':
        experiment['other_control'].append({'label': 'unexpected', 'max_bytes': 1})
    elif case == 'duplicate_label':
        experiment['runs'][9]['raw_run']['label'] = experiment['runs'][0]['raw_run']['label']
    elif case == 'global_size':
        experiment['capture_policy']['metrics_body_bytes'] = 16 * 1024 * 1024
    elif case == 'number':
        experiment['runs'][9]['raw_run']['max_bytes'] = '1024'
    elif case == 'missing_static':
        experiment['static_files'].pop()
    with pytest.raises(config.ExperimentConfigError):
        config.decode_fixed_inputs(plan_raw, encoded(value))


def test_budget_duplicates_oversize_and_depth_fail_before_existing_budget_parser(monkeypatch):
    plan_raw, budget_raw = inputs()
    def forbidden(*args, **kwargs):
        raise AssertionError('malformed JSON must not reach budget schema parser')
    monkeypatch.setattr(config, 'parse_run_budget', forbidden)
    duplicate = budget_raw[:-1] + b',"pair_index":1}'
    for raw in (duplicate, budget_raw + b' ' * config.MAX_CONFIG_BYTES,
                b'{"experiment":' + b'[' * 6 + b'0' + b']' * 6 + b'}'):
        with pytest.raises(config.ExperimentConfigError):
            config.decode_fixed_inputs(plan_raw, raw)


def test_decoded_string_and_container_bounds_are_checked_before_construction():
    plan_raw, _ = inputs()
    value = json.loads(plan_raw)
    for replacement in ('a' * 4097, '\U0001f600' * 4097, ''):
        value['seed_namespace'] = replacement
        with pytest.raises(config.ExperimentConfigError):
            config.decode_plan(encoded(value))
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(b'{"x":[' + b'0,' * 256 + b'0]}')
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(encoded({str(i): 0 for i in range(33)}))
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(encoded({'x' * 129: 0}))


def test_node_budget_precedes_schema_or_policy_construction(monkeypatch):
    raw = encoded({'x': [[0] * 256 for _ in range(16)]})
    def forbidden(*args, **kwargs):
        raise AssertionError('bounded JSON tree must precede schema construction')
    monkeypatch.setattr(config, '_object', forbidden)
    with pytest.raises(config.ExperimentConfigError):
        config.decode_plan(raw)
