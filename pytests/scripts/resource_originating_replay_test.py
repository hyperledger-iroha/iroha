"""Current API ports of historical raw replay and file-boundary outcomes.

Raw captures, journals, reductions and physical censuses use actual production
Python owners. Hooks is an explicit controlled originating-owner protocol seam;
it grants no native completion evidence and does not qualify FixedExperimentCustody
trial completion, original runtime lifetime, report publication or final release.
No child, network observation, native executable or signal runs here.
"""
from dataclasses import replace
import copy
from pathlib import Path
import shutil
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts/nexus'))
import resource_experiment as experiment
import resource_bundle as bundle
import resource_evidence_budget as budget
import resource_replay as replay
import resource_replay_test as frozen
from resource_completed_experiment_test import Hooks
from resource_originating_replay_fixture import ScopedPublishedRuns, RUNS


@pytest.fixture(scope='module')
def published(tmp_path_factory):
    return ScopedPublishedRuns(tmp_path_factory.mktemp('originating-publisher') / 'bundle')


@pytest.fixture
def valid(published, tmp_path):
    value = copy.deepcopy(published)
    value.root = (tmp_path / 'bundle').resolve()
    shutil.copytree(published.root, value.root)
    return value


def trace_raw_failure(monkeypatch):
    """Keep reducer diagnostics observable while the borrower sanitizes errors."""
    failures = []
    actual = experiment.replay

    def observed(*args, **kwargs):
        try:
            return actual(*args, **kwargs)
        except BaseException as error:
            failures.append(error)
            raise

    monkeypatch.setattr(experiment, 'replay', observed)
    return failures


def test_all_ten_actual_publishers_and_raw_replays_link_exact_allocations_and_maxima(valid, monkeypatch):
    calls = []
    actual = experiment.replay

    def observed(*args, **kwargs):
        calls.append((args, kwargs))
        return actual(*args, **kwargs)

    monkeypatch.setattr(experiment, 'replay', observed)
    hooks = Hooks(valid.scopes())
    # The physical census is tested independently. The borrower must not create
    # a replacement bundle admission or acquire the original file owner's FDs.
    with bundle.BudgetedBundle(valid.root, valid.budget, valid.controls, reported=False) as census:
        with experiment.ResourceExperiment.from_completed(hooks.owner) as owner:
            results = owner.collect_replay()
            assert tuple((row.pair_index, row.variant) for row in results) == RUNS
            assert len(calls) == 10
            for index, (result, (args, kwargs)) in enumerate(zip(results, calls, strict=True)):
                allocation = budget.select_run_budget(valid.budget, *RUNS[index])
                assert args[0] == valid.captures(index) and args[1] == valid.journal(index)
                assert args[3] == valid.runs[index].peers and args[4] == valid.geometry
                assert kwargs == {'expected_policy': allocation.policy, 'allocation': allocation}
                assert result.resources.journal_sha256 == frozen.sha(valid.journal(index).read_bytes())
                assert result.resources.journal_bytes == valid.journal(index).stat().st_size
                assert result.resources.capture_file_count == allocation.member_count == 207
                assert result.resources.capture_bytes == sum(path.stat().st_size for path in valid.captures(index).iterdir())
                assert result.resources.capture_bytes == result.resources.raw_body_bytes + result.resources.manifest_bytes
                assert result.resources.admitted_experiment_resource_file_count == 2070
                assert result.resources.admitted_resource_byte_limit == allocation.resource_bytes
                assert result.resources.admitted_journal_byte_limit == allocation.journal.max_bytes
                assert result.maxima == experiment.ResourceMaxima(410 + 40 * index, 412 + 40 * index,
                                                                 20406 + 40 * index, 806 + 40 * index)
                assert experiment.interval_maxima(result, 50_000_000, 53_000_000) == experiment.ResourceMaxima(
                    30 + 40 * index, 32 + 40 * index, 16026 + 40 * index, 426 + 40 * index)
                assert len(result.resources.samples) == 22 and result.resources.samples[-1].scheduled_offset_ns == valid.geometry.final
            assert census.snapshot.resource_files == sum(row.resources.capture_file_count for row in results) == 2070
            assert census.snapshot.resource_bytes == sum(row.resources.capture_bytes for row in results)
            assert owner.verify() is results
            census.verify()
    assert hooks.released
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'):
        owner.verify()


@pytest.mark.parametrize('change', ['missing', 'extra', 'duplicate', 'order', 'list', 'bool_pair', 'labels', 'peer_count',
    'duplicate_peer', 'geometry', 'hash', 'old_unsampled', 'bad_digest'])
def test_exact_trusted_ten_run_scope_rejected_before_bundle_io(valid, monkeypatch, change):
    rows = list(valid.scopes())
    if change == 'missing': rows.pop()
    elif change == 'extra': rows.append(rows[0])
    elif change == 'duplicate': rows[-1] = rows[0]
    elif change == 'order': rows.reverse()
    elif change == 'bool_pair': rows[0] = replace(rows[0], pair_index=True)
    elif change == 'labels': rows[5] = replace(rows[5], peers=tuple(replace(p, peer_id='other' + p.peer_id) for p in rows[5].peers))
    elif change == 'peer_count': rows[5] = replace(rows[5], peers=rows[5].peers[:3])
    elif change == 'duplicate_peer': rows[5] = replace(rows[5], peers=(rows[5].peers[0],) * 4)
    elif change == 'geometry': rows[5] = replace(rows[5], geometry=replace(rows[5].geometry, warmup_ns=101_000_000))
    elif change == 'hash': rows[5] = replace(rows[5], peers=tuple(replace(p, identity=replace(p.identity, executable_sha256='d' * 64)) for p in rows[5].peers))
    elif change == 'old_unsampled': rows = [{'pair_index': i, 'resource_observation': {}} for i in range(10)]
    elif change == 'bad_digest': rows[0] = replace(rows[0], peers=tuple(replace(p, identity=replace(p.identity, executable_sha256='x')) for p in rows[0].peers))
    scopes = rows if change == 'list' else tuple(rows)
    opened = []

    def forbidden(*args, **kwargs):
        opened.append((args, kwargs))
        raise AssertionError('invalid scope must not open bundle')

    monkeypatch.setattr(bundle, 'BudgetedBundle', forbidden)
    monkeypatch.setattr(experiment, 'replay', forbidden)
    hooks = Hooks(scopes)
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'):
        experiment.ResourceExperiment.from_completed(hooks.owner)
    assert hooks.failed and hooks.next == 0
    assert opened == []


@pytest.mark.parametrize('index', range(10))
def test_any_failed_actual_run_never_promotes_the_successful_prefix(valid, monkeypatch, index):
    valid.events[index][-1]['passed'] = False
    valid.save_journal(index)  # Refresh the selected digest; semantic rejection remains mandatory.
    failures = trace_raw_failure(monkeypatch)
    hooks = Hooks(valid.scopes())
    owner = experiment.ResourceExperiment.from_completed(hooks.owner)
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'):
        owner.collect_replay()
    assert len(failures) == 1 and isinstance(failures[0], replay.ReplayError)
    assert 'collection_failed' in str(failures[0])
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'): owner.verify()
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'): owner.collect_replay()
    assert owner._results == ()
    assert hooks.next == index and hooks.failed and not hooks.finished


@pytest.mark.parametrize('change', ['journal_pair', 'journal_variant', 'unsampled', 'missing_preflight', 'failed_finish', 'missing_last', 'identity', 'image'])
def test_complete_physical_bundle_cannot_replace_actual_replay_authority(valid, monkeypatch, change):
    index = 9
    rows = valid.events[index]
    if change == 'journal_pair': rows[0]['pair_index'] = 1
    elif change == 'journal_variant': rows[0]['variant'] = 'one_lane'
    elif change == 'unsampled': rows[:] = [rows[0], {'event': 'resource_observation', 'queue': 1}, rows[-1]]
    elif change == 'missing_preflight': rows.pop(1)
    elif change == 'failed_finish': next(row for row in rows if row['event'] == 'resource_collection_finished')['failure'] = 'late'
    elif change == 'missing_last': rows.pop(-2)
    else:
        run = valid.runs[index]
        field = 'image_uuid' if change == 'image' else 'start_abstime'
        identity = replace(run.peers[0].identity, **{field: '02' * 16 if change == 'image' else 999})
        run = replace(run, peers=(replace(run.peers[0], identity=identity), *run.peers[1:]))
        valid.runs = (*valid.runs[:index], run)
    valid.save_journal(index)
    failures = trace_raw_failure(monkeypatch)
    hooks = Hooks(valid.scopes())
    with bundle.BudgetedBundle(valid.root, valid.budget, valid.controls, reported=False) as census:
        census.verify()
        owner = experiment.ResourceExperiment.from_completed(hooks.owner)
        with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'): owner.collect_replay()
        assert len(failures) == 1 and isinstance(failures[0], replay.ReplayError)
        with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'): owner.verify()
        assert hooks.next == 9 and hooks.failed and not hooks.finished


@pytest.mark.parametrize('change', ['raw_missing', 'raw_extra', 'journal_missing', 'journal_digest', 'control_omit', 'report_present'])
def test_physical_linkage_rejects_missing_extra_or_unbound_files(valid, change):
    if change == 'raw_missing': valid.member().unlink()
    elif change == 'raw_extra': (valid.captures(9) / 'unsampled.json').write_bytes(b'{}')
    elif change == 'journal_missing': valid.journal(9).unlink()
    elif change == 'journal_digest': valid.journal(9).write_bytes(b'{}\n')
    elif change == 'control_omit': valid.controls = valid.controls[:-1]
    elif change == 'report_present': (valid.root / 'controls/report').write_bytes(b'x')
    with pytest.raises(bundle.BundleError):
        bundle.BudgetedBundle(valid.root, valid.budget, valid.controls, reported=False)


def test_missing_and_repeated_replay_poison_the_context(valid):
    hooks = Hooks(valid.scopes())
    owner = experiment.ResourceExperiment.from_completed(hooks.owner)
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'): owner.verify()
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'): owner.collect_replay()
    assert hooks.failed and owner._results == ()
    # A fresh controlled origin is a separate unit-test instance, never a way
    # to refresh the failed original production custody.
    with pytest.raises(experiment.ExperimentError): experiment.ResourceExperiment.from_completed(hooks.owner)
    second = Hooks(valid.scopes())
    owner = experiment.ResourceExperiment.from_completed(second.owner)
    owner.collect_replay()
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'): owner.collect_replay()
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'): owner.verify()
    assert second.failed and owner._results == ()


@pytest.mark.parametrize('error', [KeyboardInterrupt, MemoryError, RuntimeError])
def test_interrupted_replay_cannot_publish_a_prefix(valid, monkeypatch, error):
    actual = experiment.replay
    calls = 0
    injected = error('injected')

    def interrupted(*args, **kwargs):
        nonlocal calls
        calls += 1
        if calls == 3: raise injected
        return actual(*args, **kwargs)

    monkeypatch.setattr(experiment, 'replay', interrupted)
    hooks = Hooks(valid.scopes())
    owner = experiment.ResourceExperiment.from_completed(hooks.owner)
    public_error = KeyboardInterrupt if error is KeyboardInterrupt else experiment.ExperimentError
    with pytest.raises(public_error) as caught: owner.collect_replay()
    assert calls == 3
    assert 'injected' not in str(caught.value)
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'): owner.verify()
    assert hooks.failed and hooks.next == 2 and owner._results == ()


@pytest.mark.parametrize('start,end', [(True, 1), (-1, 1), (1, 1), (2, 1), (0, 1 << 63), (4_000_000, 5_000_000)])
def test_invalid_or_empty_observed_window_cannot_invent_maxima(valid, start, end):
    hooks = Hooks(valid.scopes())
    with experiment.ResourceExperiment.from_completed(hooks.owner) as owner:
        result = owner.collect_replay()[0]
        with pytest.raises(experiment.ExperimentError): experiment.interval_maxima(result, start, end)


@pytest.mark.parametrize('field', ['warmup_ns', 'measurement_ns', 'drain_ns', 'preparation_ahead_ns',
                                  'interval_ns', 'response_deadline_ns', 'max_start_lag_ns'])
def test_every_time_scope_field_is_independent_and_consistent_before_scan(valid, monkeypatch, field):
    scopes = valid.scopes()
    run = scopes[9]
    run = replace(run, geometry=replace(run.geometry, **{field: getattr(run.geometry, field) + 1_000_000}))
    opened = []

    def forbidden(*args, **kwargs):
        opened.append((args, kwargs))
        raise AssertionError('time mismatch must not open')

    monkeypatch.setattr(bundle, 'BudgetedBundle', forbidden)
    monkeypatch.setattr(experiment, 'replay', forbidden)
    hooks = Hooks((*scopes[:9], run))
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'):
        experiment.ResourceExperiment.from_completed(hooks.owner)
    assert hooks.failed and hooks.next == 0
    assert opened == []


def test_journal_mapping_cannot_relabel_another_fully_bound_run(valid, monkeypatch):
    left, right = (valid.budget.runs[index].collector_journal.label for index in (0, 9))
    bindings = {item.label: item for item in valid.controls}
    valid.controls = tuple(replace(item, path=bindings[right].path, sha256=bindings[right].sha256) if item.label == left
        else replace(item, path=bindings[left].path, sha256=bindings[left].sha256) if item.label == right else item
        for item in valid.controls)
    # RunReplayScope now rejects the cross-run path before replay. The actual
    # raw reducer must independently reject the relabelled journal as before.
    scope = valid.scopes()[0]
    with bundle.BudgetedBundle(valid.root, valid.budget, valid.controls, reported=False) as census:
        census.verify()
        with pytest.raises(replay.ReplayError, match='run'):
            replay.replay(scope.capture_directory, scope.journal_path, scope.journal_sha256,
                scope.peers, scope.geometry, expected_policy=scope.allocation.policy, allocation=scope.allocation)
    calls = []
    monkeypatch.setattr(experiment, 'replay', lambda *_a, **_k: calls.append(True))
    hooks = Hooks(valid.scopes())
    with pytest.raises(experiment.ExperimentError): experiment.ResourceExperiment.from_completed(hooks.owner)
    assert hooks.failed and not calls


@pytest.mark.parametrize('delta', [0, -1])
def test_actual_shared_journal_exact_allocation_and_one_byte_below(valid, delta):
    run = valid.budget.runs[9]
    maximum = valid.journal(9).stat().st_size + delta
    runs = (*valid.budget.runs[:9], replace(run, collector_journal=replace(run.collector_journal, max_bytes=maximum)))
    admitted = budget.admit_experiment(policy=valid.budget.policy, runs=runs, static_files=valid.budget.static_files,
        manifest=valid.budget.control_budgets[0], report=valid.budget.control_budgets[1], other_control=valid.budget.control_budgets[2:])
    if delta:
        with pytest.raises(bundle.BundleError, match='file_allocation_exceeded'):
            bundle.BudgetedBundle(valid.root, admitted, valid.controls, reported=False)
    else:
        valid.budget = admitted
        hooks = Hooks(valid.scopes())
        with bundle.BudgetedBundle(valid.root, admitted, valid.controls, reported=False) as census:
            with experiment.ResourceExperiment.from_completed(hooks.owner) as owner:
                rows = owner.collect_replay()
                assert rows[9].resources.admitted_journal_byte_limit == maximum == rows[9].resources.journal_bytes
                assert len(owner.verify()) == 10
                census.verify()


def test_old_unsampled_signature_has_no_entrypoint(valid):
    with pytest.raises(experiment.ExperimentError, match='^completed_experiment_required$'):
        experiment.ResourceExperiment(valid.root, {'resource_observation': {}}, valid.controls)
    assert not hasattr(experiment, 'RunReplayInput')
    assert not hasattr(experiment.ResourceExperiment, 'read_control')
    assert not hasattr(experiment.ResourceExperiment, 'reconcile_run')


def test_resource_finish_does_not_extend_transaction_drain(valid, monkeypatch):
    rows = valid.events[9]
    final = next(row for row in rows if row['event'] == 'request_final')
    final['applied_offset_ns'] = valid.geometry.final + 1
    valid.save_journal(9)
    failures = trace_raw_failure(monkeypatch)
    hooks = Hooks(valid.scopes())
    owner = experiment.ResourceExperiment.from_completed(hooks.owner)
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'):
        owner.collect_replay()
    assert len(failures) == 1 and isinstance(failures[0], replay.ReplayError)
    assert 'transaction_drain_extended' in str(failures[0])
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'):
        owner.verify()
