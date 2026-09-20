"""Real raw resource replay plus controlled originating-owner protocol hooks.

The controlled hooks exercise this borrower's boundary. They are not a native
proof authority or a substitute for the root's actual ten-trial composition.
No child, network probe, process observation or signal runs in these tests.
"""
from dataclasses import asdict, fields, replace
import copy
from pathlib import Path
import sys
import weakref
from types import SimpleNamespace

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts/nexus'))
import resource_experiment as experiment
import resource_replay as raw_replay
import resource_replay_test as fixture
from resource_evidence_budget import canonical_run_budget_bytes, select_run_budget
from scaling_completed_authority import PublicRunProjection, _PUBLIC_RECORDS
from scaling_experiment_custody import FixedExperimentCustody


@pytest.fixture(scope='module')
def raw_inputs(tmp_path_factory):
    directory = tmp_path_factory.mktemp('completed-resource').resolve()
    values = []
    original_allocation = fixture.admitted_allocation
    for pair, variant in experiment._RUNS:
        working = directory / f'fixture-{pair}-{variant}'; working.mkdir(mode=0o700)
        def allocate(peers, geometry, policy):
            return select_run_budget(original_allocation(peers, geometry, policy).experiment, pair, variant)
        with pytest.MonkeyPatch.context() as patch:
            patch.setattr(fixture, 'admitted_allocation', allocate)
            value = fixture.Fixture(working)
        capture = directory / 'evidence' / 'resources' / f'pair-{pair:02}' / variant
        journal = directory / 'evidence' / 'runs' / f'pair-{pair:02}' / variant / 'collector.jsonl'
        capture.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        journal.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        value.directory.rename(capture); value.journal.rename(journal)
        value.directory, value.journal = capture, journal
        value.events[0].update(pair_index=pair, variant=variant); value.save()
        values.append(experiment.RunReplayScope(pair, variant, capture, journal, value.digest,
                                               value.peers, value.geometry, value.allocation))
    scopes = tuple(values)
    return SimpleNamespace(scopes=scopes, reduced=tuple(raw_replay.replay(
        row.capture_directory, row.journal_path, row.journal_sha256, row.peers, row.geometry,
        expected_policy=row.allocation.policy, allocation=row.allocation) for row in scopes))


class Hooks:
    """Explicit controlled hooks on the exact class, without constructing a trial."""
    def __init__(self, scopes):
        self.owner = object.__new__(FixedExperimentCustody)
        self.token = object(); self.scopes = scopes
        self.calls = []; self.failed = self.made = self.released = self.finished = False
        self.borrower = None; self.next = 0
        self.before_action = self.accept_action = self.finish_action = self.verify_action = self.release_action = lambda *_: None
        for name in ('create_replay', 'replay_before', 'replay_accept', 'replay_finish', 'verify_replay', 'release_replay', 'reject_replay'):
            setattr(self.owner, '_' + name, getattr(self, name))

    def check(self, borrower, token):
        assert not self.failed and not self.released
        assert borrower is self.borrower and token is self.token
        assert borrower._scope_identity() == self.pin

    def create_replay(self):
        assert not self.made and not self.failed
        self.made = True
        borrower = object.__new__(experiment.ResourceExperiment)
        borrower._initialize(self.owner, self.token, self.scopes)
        self.borrower = borrower; self.pin = borrower._scope_pin
        self.calls.append('create')
        return borrower

    def replay_before(self, borrower, token):
        self.check(borrower, token); assert borrower._phase == 'replaying'
        self.calls.append('before'); self.before_action(borrower)

    def replay_accept(self, borrower, token, index, reduced):
        self.check(borrower, token); assert index == self.next and type(reduced) is raw_replay.ReplayResult
        assert reduced.signed_requests and reduced.applied_requests
        self.calls.append(('accept', index)); self.next += 1
        row = borrower._scopes[index]
        value = dict.fromkeys(PublicRunProjection._fields, ())
        value.update(pair_index=row.pair_index, variant=row.variant,
            budget=canonical_run_budget_bytes(row.allocation),
            geometry=_PUBLIC_RECORDS[raw_replay.ReplayGeometry](*(getattr(row.geometry, field.name)
                for field in fields(raw_replay.ReplayGeometry))), resources=experiment._resource_snapshot(reduced))
        projection = PublicRunProjection(**value)
        changed = self.accept_action(borrower, index, reduced, projection)
        return projection if changed is None else changed

    def replay_finish(self, borrower, token, results):
        self.check(borrower, token); assert self.next == 10 and borrower._phase == 'finishing'
        assert results is borrower._results and len(results) == 10
        self.calls.append('finish'); self.finish_action(borrower)
        self.finished = True

    def verify_replay(self, borrower, token):
        self.check(borrower, token); assert self.finished
        self.calls.append('verify'); self.verify_action(borrower)

    def release_replay(self, borrower, token):
        self.check(borrower, token); assert self.finished
        self.calls.append('release'); self.release_action(borrower); self.released = True

    def reject_replay(self, borrower, token):
        self.failed = True; self.calls.append('reject')


def cached(monkeypatch, raw_inputs):
    rows = iter(raw_inputs.reduced)
    monkeypatch.setattr(experiment, 'replay', lambda *args, **kwargs: replace(next(rows)))


def test_ten_actual_raw_replays_join_and_keep_only_immutable_resources(raw_inputs):
    hooks = Hooks(raw_inputs.scopes)
    borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    assert borrower._scopes is not raw_inputs.scopes and borrower._scopes[0].peers is not raw_inputs.scopes[0].peers
    result = borrower.collect_replay()
    assert tuple((row.pair_index, row.variant) for row in result) == experiment._RUNS
    assert hooks.calls == ['create', 'before', *(('accept', i) for i in range(10)), 'finish']
    for index, row in enumerate(result):
        reduced = raw_inputs.reduced[index]
        assert row.resources == experiment._resource_snapshot(reduced)
        assert row.maxima == experiment._maxima((reduced.preflight, *(sample.capture for sample in reduced.samples)))
        assert not hasattr(row, 'replay') and not hasattr(row.resources, 'signed_requests')
        assert not hasattr(row.resources, 'applied_requests')
        assert experiment.interval_maxima(row, 50_000_000, 53_000_000) == experiment._maxima((reduced.samples[5].capture,))
    assert borrower.verify() is result
    borrower.close(); borrower.close()
    assert hooks.calls[-2:] == ['verify', 'release'] and hooks.released
    assert all(row.journal_path.is_file() and row.capture_directory.is_dir() for row in raw_inputs.scopes)


@pytest.mark.parametrize('owner', [None, {}, (), object(), SimpleNamespace(_create_replay=lambda: object())])
def test_no_constructor_or_nonactual_owner_can_admit(owner):
    with pytest.raises(experiment.ExperimentError): experiment.ResourceExperiment(owner)
    with pytest.raises(experiment.ExperimentError): experiment.ResourceExperiment.from_completed(owner)


@pytest.mark.parametrize('change', ['missing', 'order', 'key', 'path', 'digest', 'peers', 'geometry', 'allocation'])
def test_malformed_original_scope_is_rejected_before_io(raw_inputs, monkeypatch, change):
    scopes = raw_inputs.scopes; first = scopes[0]
    if change == 'missing': scopes = scopes[:-1]
    elif change == 'order': scopes = tuple(reversed(scopes))
    else:
        value = {'key': replace(first, pair_index=True), 'path': replace(first, journal_path=Path('relative')),
            'digest': replace(first, journal_sha256='bad'), 'peers': replace(first, peers=first.peers[:-1]),
            'geometry': replace(first, geometry=replace(first.geometry, interval_ns=0)),
            'allocation': replace(first, allocation=raw_inputs.scopes[1].allocation)}[change]
        scopes = (value, *scopes[1:])
    monkeypatch.setattr(experiment, 'replay', lambda *_args, **_kwargs: pytest.fail('no evidence I/O'))
    hooks = Hooks(scopes)
    with pytest.raises(experiment.ExperimentError): experiment.ResourceExperiment.from_completed(hooks.owner)
    assert hooks.failed and hooks.next == 0


@pytest.mark.parametrize('point', ['before', 'accept', 'finish', 'verify', 'release'])
def test_hook_failure_poison_is_permanent(raw_inputs, monkeypatch, point):
    cached(monkeypatch, raw_inputs); hooks = Hooks(raw_inputs.scopes)
    borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    def fail(*_args): raise ValueError('private runtime details must not escape')
    setattr(hooks, point + '_action', fail)
    with pytest.raises(experiment.ExperimentError, match='^resource_experiment_failed$'):
        borrower.collect_replay(); borrower.verify(); borrower.close()
    assert hooks.failed and borrower._phase == 'failed' and borrower._results == ()
    with pytest.raises(experiment.ExperimentError): borrower.verify()
    with pytest.raises(experiment.ExperimentError): experiment.ResourceExperiment.from_completed(hooks.owner)


@pytest.mark.parametrize('change', ['pair', 'variant', 'budget', 'geometry', 'resources', 'foreign'])
def test_accepted_projection_must_match_actual_scope_and_reductions(raw_inputs, monkeypatch, change):
    cached(monkeypatch, raw_inputs); hooks = Hooks(raw_inputs.scopes)
    def wrong(_borrower, _index, _reduced, projection):
        if change == 'foreign': return SimpleNamespace(resources=projection.resources)
        field, value = {'pair': ('pair_index', True), 'variant': ('variant', 'four_lane'),
            'budget': ('budget', b'foreign'), 'geometry': ('geometry', ()),
            'resources': ('resources', projection.resources._replace(journal_bytes=projection.resources.journal_bytes + 1))}[change]
        return projection._replace(**{field: value})
    hooks.accept_action = wrong
    borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    with pytest.raises(experiment.ExperimentError): borrower.collect_replay()
    assert hooks.failed and hooks.next == 1 and borrower._results == ()


@pytest.mark.parametrize('change', ['owner', 'token', 'scope', 'reentry', 'finish_results', 'verify_results'])
def test_aliases_and_reentry_cannot_refresh_borrowed_authority(raw_inputs, monkeypatch, change):
    cached(monkeypatch, raw_inputs); hooks = Hooks(raw_inputs.scopes)
    borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    if change == 'owner': borrower._owner = object()
    elif change == 'token': borrower._token = object()
    elif change == 'scope': object.__setattr__(borrower._scopes[0].geometry, 'warmup_ns', 1)
    elif change == 'reentry':
        def reenter(current):
            with pytest.raises(experiment.ExperimentError): current.collect_replay()
        hooks.before_action = reenter
    else:
        def mutate(current): object.__setattr__(current._results[0].maxima, 'queue_depth_max', 999)
        setattr(hooks, 'finish_action' if change == 'finish_results' else 'verify_action', mutate)
    with pytest.raises(experiment.ExperimentError): borrower.collect_replay(); borrower.verify()
    assert hooks.failed and borrower._results == ()


def test_scope_copies_prevent_caller_alias_changes_and_readmission(raw_inputs, monkeypatch):
    scopes = tuple(replace(row, geometry=replace(row.geometry)) for row in raw_inputs.scopes)
    hooks = Hooks(scopes); borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    pin = borrower._scope_pin
    object.__setattr__(scopes[0].geometry, 'warmup_ns', 1)
    assert borrower._scope_identity() == pin
    with pytest.raises(experiment.ExperimentError): borrower._initialize(hooks.owner, hooks.token, raw_inputs.scopes)
    assert hooks.failed and borrower._phase == 'failed'


def test_reject_hook_reentry_does_not_recurse(raw_inputs, monkeypatch):
    hooks = Hooks(raw_inputs.scopes); borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    calls = []
    def reject(current, _token):
        calls.append(True)
        current.verify()
    hooks.owner._reject_replay = reject
    with pytest.raises(experiment.ExperimentError): borrower.verify()
    assert calls == [True] and borrower._phase == 'failed'


def test_measurement_math_remains_exact_and_empty_windows_fail(raw_inputs):
    reduced = raw_inputs.reduced[0]
    resources = experiment._resource_snapshot(reduced)
    maxima = experiment._maxima((reduced.preflight, *(sample.capture for sample in reduced.samples)))
    value = experiment.RunResourceResult(1, 'one_lane', resources, maxima)
    assert maxima == experiment.ResourceMaxima(10, 12, 4406, 406)
    assert experiment.interval_maxima(value, 49_000_000, 50_000_000) == maxima
    with pytest.raises(experiment.ExperimentError): experiment.interval_maxima(value, 54_000_000, 59_000_000)
    with pytest.raises(experiment.ExperimentError): experiment._maxima(())
    assert experiment._report_offset_ns(0.000000001) == 1
    with pytest.raises(experiment.ExperimentError): experiment._report_offset_ns(0.0000000001)


def test_incomplete_close_and_exception_context_poison_original_owner(raw_inputs):
    for action in ('close', 'context'):
        hooks = Hooks(raw_inputs.scopes)
        borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
        with pytest.raises(experiment.ExperimentError):
            if action == 'close': borrower.close()
            else:
                with borrower:
                    raise ValueError('private body')
        assert hooks.failed and borrower._phase == 'failed' and hooks.next == 0


def test_cloned_instance_cannot_borrow_registered_authority(raw_inputs, monkeypatch):
    hooks = Hooks(raw_inputs.scopes)
    borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    clone = object.__new__(experiment.ResourceExperiment)
    clone.__dict__.update(borrower.__dict__)
    monkeypatch.setattr(experiment, 'replay', lambda *_args, **_kwargs: pytest.fail('no evidence I/O'))
    with pytest.raises(experiment.ExperimentError): clone.collect_replay()
    assert hooks.failed and clone._phase == 'failed'
    with pytest.raises(experiment.ExperimentError): borrower.collect_replay()


def test_original_deadline_failure_precedes_evidence_io(raw_inputs, monkeypatch):
    hooks = Hooks(raw_inputs.scopes)
    borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    def expired(_borrower): raise TimeoutError('original experiment deadline expired')
    hooks.before_action = expired
    monkeypatch.setattr(experiment, 'replay', lambda *_args, **_kwargs: pytest.fail('no evidence I/O'))
    with pytest.raises(experiment.ExperimentError): borrower.collect_replay()
    assert hooks.failed and hooks.next == 0


def test_keyboard_interrupt_notifies_origin_and_keeps_failed_state(raw_inputs):
    hooks = Hooks(raw_inputs.scopes)
    borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    def interrupted(_borrower): raise KeyboardInterrupt('private body')
    hooks.before_action = interrupted
    with pytest.raises(KeyboardInterrupt) as error: borrower.collect_replay()
    assert not error.value.args and hooks.failed and borrower._phase == 'failed'


def constant_report(geometry):
    """Independent constant fixture expectation, including interval boundaries."""
    maxima = dict(queue_depth_max=10, index_entries_max=12, memory_bytes_max=4406, disk_bytes_max=406)
    def phase(begin, end):
        return dict(samples=[dict(sequence=index, start_offset_seconds=start / 1_000_000_000,
            end_offset_seconds=(start + geometry.interval_ns) / 1_000_000_000,
            **{key.removesuffix('_max'): value for key, value in maxima.items()})
            for index, start in enumerate(range(begin, end, geometry.interval_ns), 1)], summary=maxima.copy())
    return dict(pair_index=1, variant='one_lane', **phase(0, geometry.measurement_ns),
                drain=phase(geometry.measurement_ns, geometry.final)), maxima


def test_pure_report_reconciliation_uses_joined_resources_without_mutating_input(raw_inputs):
    reduced, geometry = raw_inputs.reduced[0], raw_inputs.scopes[0].geometry
    raw, maxima = constant_report(geometry); original = copy.deepcopy(raw)
    value = experiment.RunResourceResult(1, 'one_lane', experiment._resource_snapshot(reduced),
                                         experiment.ResourceMaxima(**maxima))
    assert asdict(experiment._reconcile_run_resources(value, geometry, raw, maxima)) == maxima
    assert raw == original
    assert not hasattr(experiment.ResourceExperiment, 'reconcile_run')
    assert not hasattr(experiment.ResourceExperiment, 'read_control')
    assert not hasattr(experiment, 'RunReplayInput')


@pytest.mark.parametrize('case', ['sample', 'summary', 'budget', 'schedule', 'count', 'maxima'])
def test_pure_report_comparison_rejects_changed_reductions(raw_inputs, case):
    reduced, geometry = raw_inputs.reduced[0], raw_inputs.scopes[0].geometry
    raw, maxima = constant_report(geometry)
    value = experiment.RunResourceResult(1, 'one_lane', experiment._resource_snapshot(reduced),
                                         experiment.ResourceMaxima(**maxima))
    if case == 'sample': raw['samples'][0]['disk_bytes'] += 1
    elif case == 'summary': raw['drain']['summary']['queue_depth_max'] += 1
    elif case == 'budget': maxima['memory_bytes_max'] -= 1
    elif case == 'schedule':
        rows = (value.resources.samples[0]._replace(scheduled_offset_ns=1), *value.resources.samples[1:])
        value = replace(value, resources=value.resources._replace(samples=rows))
    elif case == 'count': value = replace(value, resources=value.resources._replace(samples=value.resources.samples[:-1]))
    elif case == 'maxima': value = replace(value, maxima=replace(value.maxima, disk_bytes_max=1))
    with pytest.raises(experiment.ExperimentError): experiment._reconcile_run_resources(value, geometry, raw, maxima)


def test_full_replay_owner_is_released_before_next_run(raw_inputs, monkeypatch):
    references = []
    def next_original(*_args, **_kwargs):
        # The raw owner contains all signed bodies. No previous raw owner may
        # survive the completed join when the next bounded replay begins.
        assert all(reference() is None for reference in references)
        value = replace(raw_inputs.reduced[len(references)])
        references.append(weakref.ref(value))
        return value
    monkeypatch.setattr(experiment, 'replay', next_original)
    hooks = Hooks(raw_inputs.scopes)
    borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    borrower.collect_replay()
    assert len(references) == 10 and all(reference() is None for reference in references)
    assert borrower.verify() is borrower._results
    borrower.close()


@pytest.mark.parametrize('field', ['_results', '_results_pin'])
@pytest.mark.parametrize('empty', [(), [], {}, None, False])
@pytest.mark.parametrize('operation', ['verify', 'close'])
def test_cleared_completed_results_cannot_skip_original_pin(raw_inputs, monkeypatch, field, empty, operation):
    cached(monkeypatch, raw_inputs); hooks = Hooks(raw_inputs.scopes)
    borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    borrower.collect_replay()
    setattr(borrower, field, empty)
    before = tuple(hooks.calls)
    with pytest.raises(experiment.ExperimentError): getattr(borrower, operation)()
    assert hooks.failed and borrower._phase == 'failed' and borrower._results == ()
    assert hooks.calls == [*before, 'reject']


@pytest.mark.parametrize('field', ['_results', '_results_pin'])
@pytest.mark.parametrize('phase', ['admitted', 'replaying'])
def test_uncollected_phases_require_exact_empty_results(raw_inputs, monkeypatch, field, phase):
    hooks = Hooks(raw_inputs.scopes)
    borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    def mutate(current): setattr(current, field, (object(),))
    if phase == 'admitted': mutate(borrower)
    else: hooks.before_action = mutate
    monkeypatch.setattr(experiment, 'replay', lambda *_args, **_kwargs: pytest.fail('no evidence I/O'))
    with pytest.raises(experiment.ExperimentError): borrower.collect_replay()
    assert hooks.failed and hooks.next == 0 and borrower._results == ()


@pytest.mark.parametrize('field', ['pair_index', 'variant'])
def test_returned_result_key_type_is_checked_before_comparison(raw_inputs, monkeypatch, field):
    cached(monkeypatch, raw_inputs); hooks = Hooks(raw_inputs.scopes)
    borrower = experiment.ResourceExperiment.from_completed(hooks.owner)
    borrower.collect_replay()
    calls = []
    class Foreign:
        def __eq__(self, other):
            calls.append(other)
            raise AssertionError('foreign equality must not execute')
    object.__setattr__(borrower._results[0], field, Foreign())
    with pytest.raises(experiment.ExperimentError): borrower.verify()
    assert not calls and hooks.failed and borrower._phase == 'failed'
