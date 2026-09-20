"""Pure ten-run projections; fixture values establish no native authority."""
from dataclasses import fields, replace
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/nexus'))
sys.path.insert(0, str(ROOT / 'scripts/tests'))
import scaling_experiment_final_projection as final
from resource_bundle import ControlBinding
from resource_evidence_budget import (RUN_FILE_FIELDS, MAX_FILE_BYTES, FileBudget,
    admit_experiment, select_run_budget, run_budget_inputs)
from resource_experiment import ResourceMaxima
from scaling_experiment_plan import RUN_KEYS, plan_bytes
from scaling_public_files import PublicFile, _PATHS
from scaling_trial_captures import CaptureCensus
from scaling_measurements import ExperimentMeasurements, RunMeasurements, measure_experiment
from scaling_measurements_test import matrix
from scaling_experiment_custody_test import fixed_plan, budget_for


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


def manifest():
    plan = fixed_plan()
    budget = budget_for(plan)
    start = 1_000_000_000_000
    deadline = start + plan.experiment_timeout_ns
    inputs = tuple(ControlBinding(role, f'inputs/{role}.json',
        hashlib.sha256(plan_bytes(plan)).hexdigest() if role == 'plan' else digest(role))
        for role in ('identity', 'plan', 'source_closure'))
    runs = []
    for index, (pair, variant) in enumerate(RUN_KEYS):
        allocation = select_run_budget(budget, pair, variant)
        rows = tuple(PublicFile(role, ControlBinding(getattr(allocation.run, role).label,
            f'runs/pair-{pair:02}/{variant}/{_PATHS[role]}', digest(f'{pair}:{variant}:{role}')),
            7, getattr(allocation.run, role).max_bytes) for role in RUN_FILE_FIELDS)
        census = CaptureCensus(allocation.member_count, 1000, digest(f'census:{pair}:{variant}'),
                               digest(f'metadata:{pair}:{variant}'))
        runs.append(final.RunManifest(pair, variant, start + plan.trial_timeout_ns + index * 1_000_000_000,
                                      rows, census))
    return plan, budget, deadline, inputs, tuple(runs)


@pytest.fixture
def observed():
    plan, values = matrix()
    return measure_experiment(plan, values)


def test_complete_manifest_preserves_exact_fifteen_artifacts_and_all_censuses():
    args = manifest()
    raw = final.manifest_bytes(*args, final.MAX_PROJECTION_BYTES)
    value = json.loads(raw)
    assert value['schema'] == final.MANIFEST_SCHEMA
    assert value['original_deadline_ns'] == args[2]
    assert value['plan_sha256'] == hashlib.sha256(plan_bytes(args[0])).hexdigest()
    assert value['budget'] == run_budget_inputs(select_run_budget(args[1], 1, 'one_lane'))['experiment']
    assert tuple((row['pair_index'], row['variant']) for row in value['runs']) == RUN_KEYS
    assert [row['label'] for row in value['inputs']] == ['identity', 'plan', 'source_closure']
    assert len({row['path'] for run in value['runs'] for row in run['files']}) == 150
    for original, row in zip(args[4], value['runs'], strict=True):
        assert tuple(item['role'] for item in row['files']) == RUN_FILE_FIELDS
        assert row['captures']['census_sha256'] == original.census.census_sha256
        assert row['captures']['metadata_sha256'] == original.census.metadata_sha256
        assert row['captures']['files'] == original.census.files
        assert row['captures']['bytes'] == original.census.bytes
        for source, item in zip(original.files, row['files'], strict=True):
            assert (item['sha256'], item['bytes'], item['max_bytes']) == (source.binding.sha256, source.bytes, source.max_bytes)
    assert b'private' not in raw and b'PASS' not in raw and b'qualification' not in raw
    assert final.manifest_bytes(*args, len(raw)) == raw
    with pytest.raises(final.FinalProjectionError):
        final.manifest_bytes(*args, len(raw) - 1)


@pytest.mark.parametrize('case', ['missing', 'extra', 'order', 'pair_bool', 'variant', 'foreign',
    'deadline_after', 'deadline_equal', 'deadline_before', 'deadline_repeat', 'deadline_bool', 'files_missing',
    'files_order', 'file_foreign', 'path_private', 'path_traversal', 'label', 'digest',
    'file_size', 'file_cap', 'census_count', 'census_bytes', 'census_hash', 'input_order', 'input_plan', 'input_private'])
def test_manifest_rejects_nonoriginal_shape_order_paths_deadlines_and_caps(case):
    plan, budget, deadline, inputs, runs = manifest()
    first = runs[0]
    if case == 'missing': runs = runs[:-1]
    elif case == 'extra': runs = (*runs, first)
    elif case == 'order': runs = (runs[1], first, *runs[2:])
    elif case == 'pair_bool': first = replace(first, pair_index=True)
    elif case == 'variant': first = replace(first, variant='four_lane')
    elif case == 'foreign': first = object()
    elif case == 'deadline_after': first = replace(first, original_deadline_ns=deadline + 1)
    elif case == 'deadline_equal': first = replace(first, original_deadline_ns=deadline)
    elif case == 'deadline_before': first = replace(first, original_deadline_ns=deadline - plan.experiment_timeout_ns)
    elif case == 'deadline_repeat': runs = (first, replace(runs[1], original_deadline_ns=first.original_deadline_ns), *runs[2:])
    elif case == 'deadline_bool': first = replace(first, original_deadline_ns=True)
    elif case == 'files_missing': first = replace(first, files=first.files[:-1])
    elif case == 'files_order': first = replace(first, files=tuple(reversed(first.files)))
    elif case == 'file_foreign': first = replace(first, files=(object(), *first.files[1:]))
    elif case in ('path_private', 'path_traversal', 'label', 'digest', 'file_size', 'file_cap'):
        item = first.files[0]
        if case.startswith('path'):
            binding = replace(item.binding)
            object.__setattr__(binding, 'path', '/private/config.toml' if case == 'path_private' else '../secret')
            item = replace(item, binding=binding)
        elif case == 'label': item = replace(item, binding=replace(item.binding, label='another'))
        elif case == 'digest':
            binding = replace(item.binding); object.__setattr__(binding, 'sha256', 'A' * 64)
            item = replace(item, binding=binding)
        elif case == 'file_size': item = replace(item, bytes=item.max_bytes + 1)
        else: item = replace(item, max_bytes=item.max_bytes + 1)
        first = replace(first, files=(item, *first.files[1:]))
    elif case == 'census_count': first = replace(first, census=replace(first.census, files=first.census.files + 1))
    elif case == 'census_bytes': first = replace(first, census=replace(first.census, bytes=1 << 63))
    elif case == 'census_hash': first = replace(first, census=replace(first.census, metadata_sha256='bad'))
    elif case == 'input_order': inputs = tuple(reversed(inputs))
    elif case == 'input_plan': inputs = (inputs[0], replace(inputs[1], sha256=digest('wrong-plan')), inputs[2])
    elif case == 'input_private':
        item = replace(inputs[0]); object.__setattr__(item, 'path', '/private/identity.json')
        inputs = (item, *inputs[1:])
    if first is not runs[0] and case not in ('missing', 'extra', 'order', 'deadline_repeat'):
        runs = (first, *runs[1:])
    with pytest.raises(final.FinalProjectionError, match='^scaling_final_projection_invalid$'):
        final.manifest_bytes(plan, budget, deadline, inputs, runs, final.MAX_PROJECTION_BYTES)


def test_budget_copy_is_precharged_before_original_plan_or_canonical_budget_projection(monkeypatch):
    args = manifest()
    def forbidden(*args, **kwargs): raise AssertionError('unadmitted allocation')
    monkeypatch.setattr(final, 'admit_plan', forbidden)
    monkeypatch.setattr(final, 'run_budget_inputs', forbidden)
    with pytest.raises(final.FinalProjectionError): final.manifest_bytes(*args, 1)


def test_larger_admitted_reservations_keep_intrinsically_bounded_output(observed, monkeypatch):
    plan, budget, deadline, inputs, runs = manifest()
    admitted = admit_experiment(policy=budget.policy, runs=budget.runs, static_files=budget.static_files,
        manifest=FileBudget('manifest', MAX_FILE_BYTES), report=FileBudget('report', MAX_FILE_BYTES), other_control=())
    manifest_raw = final.manifest_bytes(plan, admitted, deadline, inputs, runs, MAX_FILE_BYTES)
    report_raw = final.report_bytes(hashlib.sha256(manifest_raw).hexdigest(), observed, MAX_FILE_BYTES)
    assert len(manifest_raw) < final.MAX_PROJECTION_BYTES and len(report_raw) < final.MAX_PROJECTION_BYTES
    assert json.loads(manifest_raw)['budget']['manifest']['max_bytes'] == MAX_FILE_BYTES
    assert json.loads(manifest_raw)['budget']['report']['max_bytes'] == MAX_FILE_BYTES
    monkeypatch.setattr(final, 'MAX_PROJECTION_BYTES', 64)
    with pytest.raises(final.FinalProjectionError): final.report_bytes(digest('manifest'), observed, MAX_FILE_BYTES)


def test_report_is_exact_observed_scope_and_has_no_latency_array_or_combined_verdict(observed):
    raw = final.report_bytes(digest('manifest'), observed, final.MAX_PROJECTION_BYTES)
    value = json.loads(raw)
    assert value['scope'] == 'observed_measurements' and value['schema'] == final.REPORT_SCHEMA
    assert value['manifest_sha256'] == digest('manifest') and len(value['runs']) == 10
    assert value['median_throughput_ratio'] == {'numerator': 1, 'denominator': 1}
    assert value['pooled_p95_latency_ratio'] == {'numerator': 1, 'denominator': 1}
    assert value['throughput_criterion_met'] is False
    assert value['latency_criterion_met'] is True and value['observed_resource_criterion_met'] is True
    assert value['criteria']['minimum_median_throughput_ratio'] == {'numerator': 3, 'denominator': 2}
    assert value['criteria']['maximum_pooled_p95_latency_ratio'] == {'numerator': 5, 'denominator': 4}
    assert value['criteria']['minimum_latency_samples_per_run'] == 100
    assert not ({'PASS', 'pass', 'qualified', 'qualification', 'success', 'result'} & value.keys())
    assert all('latencies_ns' not in row and row['warmup_p95_latency_ns'] is None for row in value['runs'])
    assert final.report_bytes(digest('manifest'), observed, len(raw)) == raw
    with pytest.raises(final.FinalProjectionError): final.report_bytes(digest('manifest'), observed, len(raw) - 1)


def test_fraction_is_preserved_without_float_conversion(observed):
    first = replace(observed.runs[0], offered_load_tps=Fraction(801, 8), committed_throughput_tps=Fraction(100, 3))
    value = replace(observed, runs=(first, *observed.runs[1:]), median_throughput_ratio=Fraction(3, 2),
                    pooled_p95_latency_ratio=Fraction(5, 4))
    raw = json.loads(final.report_bytes(digest('manifest'), value, final.MAX_PROJECTION_BYTES))
    assert raw['runs'][0]['committed_throughput_tps'] == {'numerator': 100, 'denominator': 3}
    assert raw['runs'][0]['offered_load_tps'] == {'numerator': 801, 'denominator': 8}
    assert raw['median_throughput_ratio'] == {'numerator': 3, 'denominator': 2}
    assert raw['pooled_p95_latency_ratio'] == {'numerator': 5, 'denominator': 4}


def test_measurement_identity_detaches_all_mutable_records_and_fraction_internals(observed):
    pin = final.measurement_identity(observed)
    assert len(pin) == len(fields(ExperimentMeasurements))
    assert len(pin[0][0]) == len(fields(RunMeasurements))
    assert pin[0][0][10] is observed.runs[0].latencies_ns
    object.__setattr__(observed.runs[0].observed_resources, 'queue_depth_max', 99)
    assert final.measurement_identity(observed) != pin and pin[0][0][11][0] != 99
    original = pin[1]
    object.__setattr__(observed.one_lane_median_throughput_tps, '_numerator', 123)
    assert final.measurement_identity(observed) != pin and pin[1] == original


@pytest.mark.parametrize('name', [field.name for field in fields(ExperimentMeasurements)])
def test_every_experiment_measurement_field_is_present_in_the_immutable_identity(observed, name):
    pin = final.measurement_identity(observed)
    old = getattr(observed, name)
    if name == 'runs':
        changed = (replace(old[0], p95_latency_ns=old[0].p95_latency_ns + 1), *old[1:])
    elif type(old) is Fraction:
        changed = old + Fraction(1, 7)
    elif type(old) is bool:
        changed = not old
    elif type(old) is ResourceMaxima:
        changed = replace(old, queue_depth_max=old.queue_depth_max + 1)
    else:
        changed = old + 1
    assert final.measurement_identity(replace(observed, **{name: changed})) != pin


@pytest.mark.parametrize('name', [field.name for field in fields(RunMeasurements)])
def test_every_run_measurement_field_is_pinned_or_rejected_on_mutation(observed, name):
    pin = final.measurement_identity(observed)
    row = observed.runs[0]
    old = getattr(row, name)
    if type(old) is Fraction: changed = old + Fraction(1, 7)
    elif type(old) is bool: changed = not old
    elif type(old) is ResourceMaxima: changed = replace(old, index_entries_max=old.index_entries_max + 1)
    elif type(old) is tuple: changed = (old[0] + 1, *old[1:])
    elif type(old) is str: changed = 'four_lane'
    elif old is None: changed = 1
    else: changed = old + 1
    value = replace(observed, runs=(replace(row, **{name: changed}), *observed.runs[1:]))
    try:
        actual = final.measurement_identity(value)
    except final.FinalProjectionError:
        return
    assert actual != pin


def test_fraction_mutated_to_a_foreign_digest_like_scalar_is_rejected_before_equality(observed):
    calls = []
    class Integer(int):
        def __eq__(self, other): calls.append('eq'); return True
    ratio = Fraction(1, 2)
    object.__setattr__(ratio, '_denominator', Integer(2))
    with pytest.raises(final.FinalProjectionError):
        final.measurement_identity(replace(observed, median_throughput_ratio=ratio))
    assert calls == []


@pytest.mark.parametrize('case', ['foreign', 'subclass', 'runs_list', 'runs_oversize', 'runs_order',
    'row_foreign', 'pair_bool', 'variant_subclass', 'rate_float', 'fraction_subclass', 'fraction_huge',
    'fraction_zero_denominator', 'fraction_not_reduced', 'latencies_list', 'latencies_oversize',
    'latency_bool', 'latency_zero', 'latency_huge', 'count_small', 'count_drift', 'warmup_drift',
    'warmup_none', 'count_total', 'flag_int', 'resource_foreign', 'resource_huge', 'resource_bool',
    'ratio_float', 'global_flag_int'])
def test_measurement_identity_rejects_closed_shape_and_preallocation_bounds(observed, case):
    row = observed.runs[0]
    class Text(str): pass
    class Ratio(Fraction): pass
    class Measurements(ExperimentMeasurements): pass
    if case == 'foreign': observed = object()
    elif case == 'subclass': observed = Measurements(*(getattr(observed, f.name) for f in fields(ExperimentMeasurements)))
    elif case == 'runs_list': observed = replace(observed, runs=list(observed.runs))
    elif case == 'runs_oversize': observed = replace(observed, runs=observed.runs * 10000)
    elif case == 'runs_order': observed = replace(observed, runs=tuple(reversed(observed.runs)))
    elif case == 'row_foreign': row = object()
    elif case == 'pair_bool': row = replace(row, pair_index=True)
    elif case == 'variant_subclass': row = replace(row, variant=Text('one_lane'))
    elif case == 'rate_float': row = replace(row, offered_load_tps=1.0)
    elif case == 'fraction_subclass': row = replace(row, offered_load_tps=Ratio(1))
    elif case == 'fraction_huge': row = replace(row, offered_load_tps=Fraction(1 << 256))
    elif case in ('fraction_zero_denominator', 'fraction_not_reduced'):
        rate = Fraction(3, 2)
        object.__setattr__(rate, '_denominator', 0 if case == 'fraction_zero_denominator' else 3)
        row = replace(row, offered_load_tps=rate)
    elif case == 'latencies_list': row = replace(row, latencies_ns=list(row.latencies_ns))
    elif case == 'latencies_oversize': row = replace(row, latencies_ns=(1,) * 65537)
    elif case in ('latency_bool', 'latency_zero', 'latency_huge'):
        value = {'latency_bool': True, 'latency_zero': 0, 'latency_huge': 1 << 63}[case]
        row = replace(row, latencies_ns=(value, *row.latencies_ns[1:]))
    elif case == 'count_small': row = replace(row, measurement_requests=99)
    elif case == 'count_drift': row = replace(row, drain_committed=1)
    elif case == 'warmup_drift': row = replace(row, warmup_p95_latency_ns=1)
    elif case == 'warmup_none': row = replace(row, warmup_requests=1)
    elif case == 'count_total': row = replace(row, warmup_requests=65536, warmup_p95_latency_ns=1)
    elif case == 'flag_int': row = replace(row, observed_resource_limits_met=1)
    elif case == 'resource_foreign': row = replace(row, observed_resources=object())
    elif case in ('resource_huge', 'resource_bool'):
        row = replace(row, observed_resources=replace(row.observed_resources, memory_bytes_max=True if case == 'resource_bool' else (1 << 53) + 1))
    elif case == 'ratio_float': observed = replace(observed, median_throughput_ratio=1.0)
    elif case == 'global_flag_int': observed = replace(observed, throughput_criterion_met=0)
    if case not in ('foreign', 'subclass', 'runs_list', 'runs_oversize', 'runs_order', 'ratio_float', 'global_flag_int'):
        observed = replace(observed, runs=(row, *observed.runs[1:]))
    with pytest.raises(final.FinalProjectionError): final.measurement_identity(observed)


def test_foreign_getters_and_forged_scalar_equality_never_execute(observed):
    events = []
    class Foreign:
        def __getattribute__(self, name): events.append(name); raise AssertionError('getter')
        def __eq__(self, other): events.append('equality'); return True
        def __iter__(self): events.append('iterator'); raise AssertionError('iterator')
    with pytest.raises(final.FinalProjectionError): final.measurement_identity(Foreign())
    row = replace(observed.runs[0], variant=Foreign())
    with pytest.raises(final.FinalProjectionError): final.measurement_identity(replace(observed, runs=(row, *observed.runs[1:])))
    ratio = Fraction(1, 2); object.__setattr__(ratio, '_numerator', Foreign())
    with pytest.raises(final.FinalProjectionError): final.measurement_identity(replace(observed, median_throughput_ratio=ratio))
    assert events == []


def test_full_supported_cohorts_keep_report_size_independent_of_latency_count(observed):
    rows = tuple(replace(row, measurement_requests=65536, measurement_committed=65536,
        drain_committed=0, latencies_ns=(1,) * 65536, p95_latency_ns=1) for row in observed.runs)
    value = replace(observed, runs=rows)
    pin = final.measurement_identity(value)
    assert sum(len(row[10]) for row in pin[0][::2]) == 327680
    assert sum(len(row[10]) for row in pin[0][1::2]) == 327680
    assert len(final.report_bytes(digest('manifest'), value, final.MAX_PROJECTION_BYTES)) < 16000


@pytest.mark.parametrize('cap', [True, 0, -1, 1 << 63, 1.0])
def test_report_cap_is_an_exact_original_bounded_integer(observed, cap):
    with pytest.raises(final.FinalProjectionError): final.report_bytes(digest('manifest'), observed, cap)
