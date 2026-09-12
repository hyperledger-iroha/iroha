"""Compare report resources against actual independently expected publisher data."""
from dataclasses import asdict, replace
import copy
from pathlib import Path
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts/nexus'))
import resource_experiment as experiment
import resource_experiment_test as integration

FIELDS = ('queue_depth_max', 'index_entries_max', 'memory_bytes_max', 'disk_bytes_max')
EXPECTED = dict(zip(FIELDS, (410, 412, 20406, 806)))


@pytest.fixture(scope='module')
def actual(tmp_path_factory):
    published = integration.PublishedExperiment(tmp_path_factory.mktemp('reconcile') / 'bundle')
    with published.owner() as owner:
        results = owner.collect_replay()
        owner.verify()
    # Independent closed-form values from the four-peer synthetic publisher:
    # status i+1+b, index3+b, disk100+i+b, RSS4000+i+b. Preflight
    # b=100 and RSS5000+i+b dominate whole-run maxima (i=0..3).
    assert asdict(results[0].maxima) == EXPECTED
    return results[0], published.geometry


def report():
    def phase(start, count):
        samples = []
        for sequence in range(1, count + 1):
            boost = start + sequence  # Inclusive right endpoint includes this capture.
            samples.append(dict(sequence=sequence, start_offset_seconds=(boost - 1) / 100,
                end_offset_seconds=boost / 100, queue_depth=10 + 4 * boost,
                index_entries=12 + 4 * boost, memory_bytes=16006 + 4 * boost,
                disk_bytes=406 + 4 * boost))
        return dict(samples=samples, summary={key: samples[-1][key.removesuffix('_max')] for key in FIELDS})
    return dict(pair_index=1, variant='one_lane', **phase(0, 20), drain=phase(20, 1))


def test_actual_raw_replay_requires_exact_reports_and_preserves_every_input(actual):
    result, geometry = actual
    raw = report()
    original = copy.deepcopy(raw)
    assert asdict(experiment._reconcile_run_resources(result, geometry, raw, EXPECTED)) == EXPECTED
    assert raw == original
    # The reported measurement/drain numbers are intentionally below preflight.
    assert raw['summary']['memory_bytes_max'] < EXPECTED['memory_bytes_max']
    assert raw['drain']['summary']['disk_bytes_max'] < EXPECTED['disk_bytes_max']


@pytest.mark.parametrize('phase', ['measurement', 'drain'])
@pytest.mark.parametrize('field', FIELDS)
@pytest.mark.parametrize('target', ['sample', 'summary'])
@pytest.mark.parametrize('change', [-1, 1])
def test_each_reported_resource_must_equal_actual_capture_reduction(actual, phase, field, target, change):
    result, geometry = actual
    raw = report()
    part = raw if phase == 'measurement' else raw['drain']
    if target == 'sample':
        part['samples'][0][field.removesuffix('_max')] += change
    else:
        part['summary'][field] += change
    with pytest.raises(experiment.ExperimentError, match='resource_report_(observation|summary)_mismatch'):
        experiment._reconcile_run_resources(result, geometry, raw, EXPECTED)


@pytest.mark.parametrize('field', FIELDS)
def test_whole_run_budget_includes_preflight_even_when_every_interval_fits(actual, field):
    result, geometry = actual
    limits = EXPECTED | {field: EXPECTED[field] - 1}
    with pytest.raises(experiment.ExperimentError, match='resource_observation_exceeds_budget'):
        experiment._reconcile_run_resources(result, geometry, report(), limits)


@pytest.mark.parametrize('value', [None, True, False, '0.1', -1, float('nan'), float('inf'),
                                  -float('inf'), 0.0000000001, 1 << 63, 10 ** 100, 10_000_000_000])
def test_report_offsets_cannot_round_or_escape_the_bounded_clock(value):
    with pytest.raises(experiment.ExperimentError, match='report_offset_invalid'):
        experiment._report_offset_ns(value)


@pytest.mark.parametrize('value,expected', [(0, 0), (1, 1_000_000_000),
    (0.000000001, 1), (0.2, 200_000_000), (9_000_000_000, 9_000_000_000_000_000_000)])
def test_exact_report_offsets_have_no_float_rounding(value, expected):
    assert experiment._report_offset_ns(value) == expected


@pytest.mark.parametrize('field', FIELDS)
@pytest.mark.parametrize('invalid', [True, -1, 1.0, '1', None, (1 << 53) + 1])
def test_resource_numbers_are_exact_bounded_integers(field, invalid):
    with pytest.raises(experiment.ExperimentError, match='reported_resources_invalid'):
        experiment._reported_maxima(EXPECTED | {field: invalid})


@pytest.mark.parametrize('case', ['pair', 'variant', 'pair_bool', 'missing_drain', 'empty', 'too_many',
    'sequence', 'sequence_bool', 'gap', 'overlap', 'coarse', 'past_end', 'incomplete',
    'wrong_replay_count', 'wrong_replay_schedule', 'forged_maxima', 'zero_budget', 'bad_geometry'])
def test_reports_cannot_relabel_weaken_or_replace_the_actual_replay(actual, case):
    result, geometry = actual
    raw, limits = report(), EXPECTED.copy()
    if case == 'pair': raw['pair_index'] = 2
    elif case == 'variant': raw['variant'] = 'four_lane'
    elif case == 'pair_bool': raw['pair_index'] = True
    elif case == 'missing_drain': del raw['drain']
    elif case == 'empty': raw['samples'] = []
    elif case == 'too_many': raw['samples'] = [raw['samples'][0]] * 100_001
    elif case == 'sequence': raw['samples'][0]['sequence'] = 2
    elif case == 'sequence_bool': raw['samples'][0]['sequence'] = True
    elif case == 'gap': raw['samples'][0]['start_offset_seconds'] = 0.001
    elif case == 'overlap': raw['samples'][1]['start_offset_seconds'] = 0
    elif case == 'coarse': raw['samples'][0]['end_offset_seconds'] = 0.02
    elif case == 'past_end': raw['drain']['samples'][0]['end_offset_seconds'] = 0.22
    elif case == 'incomplete': raw['samples'].pop()
    elif case == 'wrong_replay_count': result = replace(result, replay=replace(result.replay, samples=result.replay.samples[:-1]))
    elif case == 'wrong_replay_schedule':
        rows = (replace(result.replay.samples[0], scheduled_offset_ns=1), *result.replay.samples[1:])
        result = replace(result, replay=replace(result.replay, samples=rows))
    elif case == 'forged_maxima': result = replace(result, maxima=replace(result.maxima, disk_bytes_max=1))
    elif case == 'zero_budget': limits['disk_bytes_max'] = 0
    elif case == 'bad_geometry': geometry = replace(geometry, interval_ns=0)
    with pytest.raises(ValueError):
        experiment._reconcile_run_resources(result, geometry, raw, limits)


@pytest.fixture(scope='module')
def retained_bundle(tmp_path_factory):
    return integration.PublishedExperiment(tmp_path_factory.mktemp('retained-reconcile') / 'bundle')


def test_context_owns_geometry_reconciles_before_final_scan_and_rejects_closed_use(retained_bundle):
    with retained_bundle.owner() as owner:
        result = owner.collect_replay()[0]
        assert owner.reconcile_run(1, 'one_lane', report(), EXPECTED) == result.maxima
        assert owner.verify()[0] == result
    with pytest.raises(experiment.ExperimentError, match='experiment_closed'):
        owner.reconcile_run(1, 'one_lane', report(), EXPECTED)


@pytest.mark.parametrize('case', ['before_replay', 'wrong_report', 'wrong_pair', 'wrong_variant',
    'geometry_drift', 'interrupt'])
def test_failed_context_reconciliation_cannot_be_promoted_by_a_final_scan(retained_bundle, monkeypatch, case):
    with retained_bundle.owner() as owner:
        if case != 'before_replay': owner.collect_replay()
        raw, pair, variant = report(), 1, 'one_lane'
        if case == 'wrong_report': raw['summary']['disk_bytes_max'] += 1
        elif case == 'wrong_pair': pair = 2
        elif case == 'wrong_variant': variant = 'four_lane'
        elif case == 'geometry_drift':
            # Valid changed warmup keeps final offset and every sample identical.
            # Only the retained originating scope can reject this drift.
            altered = replace(owner._runs[0], geometry=replace(owner._runs[0].geometry, warmup_ns=101_000_000))
            owner._runs = (altered, *owner._runs[1:])
        elif case == 'interrupt':
            def interrupted(*_): raise KeyboardInterrupt
            monkeypatch.setattr(experiment, '_reconcile_run_resources', interrupted)
        with pytest.raises((ValueError, KeyboardInterrupt)):
            owner.reconcile_run(pair, variant, raw, EXPECTED)
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'):
            owner.verify()


def test_internal_comparison_has_no_public_unscoped_entrypoint():
    assert not hasattr(experiment, 'reconcile_run_resources')
