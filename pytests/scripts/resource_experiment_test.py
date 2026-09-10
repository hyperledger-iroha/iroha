"""Actual ten-run publisher/replay composition on synthetic raw evidence.

Expected process scopes and source journal rows are constructed independently;
no expected identity, policy, or Clock value is read back from capture manifests.
These tests run no children, network probes, canonical validation or real trial.
"""
from dataclasses import asdict, replace
import copy
import json
from pathlib import Path
import shutil
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts/nexus'))
import resource_experiment as experiment
import resource_bundle as bundle
import resource_evidence_budget as budget
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts/tests'))
from signed_request_fixture import add_retention
import resource_replay as replay
import resource_probe as probe
import resource_probe_worker as worker
import resource_process as process
import kura_resource_metrics as metrics
import resource_replay_test as frozen

RUNS = tuple((pair, variant) for pair in range(1, 6) for variant in ('one_lane', 'four_lane'))


def observation(peers, policy, boost, preflight=False):
    rows = []
    for index, peer in enumerate(peers):
        status = frozen.encode({'queue_size': index + 1 + boost})
        raw = frozen.body(100 + index + boost)
        raw = raw.replace(b'family="resident_canonical"} 3\n',
                          f'family="resident_canonical"}} {3 + boost}\n'.encode())
        raw = raw.replace(b'resident_associations_sum 3\n',
                          f'resident_associations_sum {3 + boost}\n'.encode())
        raw = raw.replace(b'represented_entries 3\n', f'represented_entries {3 + boost}\n'.encode())
        def provenance(route, value, kind):
            return probe.HttpProvenance(route, frozen.sha(value), len(value), kind, value)
        rows.append(probe.PeerObservation(peer.peer_id, index + 1 + boost,
            provenance('/status', status, 'application/json'), provenance('/metrics', raw, 'text/plain'),
            metrics.parse_kura_resource_metrics(raw),
            process.ProcessSample(peer.identity, (5000 if preflight else 1000) + index + boost),
            process.ProcessSample(peer.identity, (1100 if preflight else 4000) + index + boost)))
    return probe.ProbeObservation(tuple(rows), 1_000_000,
        sum(len(row.status.raw_body) + len(row.metrics.raw_body) + 200 for row in rows),
        sum(row.process_before.rss_bytes for row in rows), sum(row.process_after.rss_bytes for row in rows),
        sum(row.queue_size for row in rows), max(row.queue_size for row in rows),
        probe.InventoryAggregate(sum(100 + index + boost for index in range(len(rows))), len(rows) * (3 + boost)), policy)


class PublishedExperiment:
    def __init__(self, root):
        self.root = root.resolve()
        self.root.mkdir(mode=0o700)
        self.geometry = replay.ReplayGeometry(100_000_000, 200_000_000, 10_000_000,
                                              20_000_000, 10_000_000, 4_000_000, 2_000_000)
        self.budget = frozen.admitted_allocation(4, self.geometry, budget.CapturePolicy()).experiment
        self.runs = tuple(experiment.RunReplayInput(pair, variant,
            tuple(replace(peer, identity=replace(peer.identity, pid=peer.identity.pid + index * 10,
                        start_seconds=index + 1, start_abstime=peer.identity.start_abstime + index * 10))
                  for peer in frozen.identities()), self.geometry)
            for index, (pair, variant) in enumerate(RUNS))
        self.events = []
        journals = {}
        for run_index, run in enumerate(self.runs):
            allocation = budget.select_run_budget(self.budget, run.pair_index, run.variant)
            directory = self.captures(run_index)
            directory.mkdir(mode=0o700, parents=True)
            geometry = run.geometry
            plan = {key: 1 for key in replay.PLAN_FIELDS}
            plan.update(event='plan', schema=replay.JOURNAL_SCHEMA, scheduled_requests=1,
                pair_index=run.pair_index, variant=run.variant,
                **{name: getattr(geometry, name) for name in ('warmup_ns', 'measurement_ns', 'drain_ns', 'preparation_ahead_ns')})
            events = [plan]
            # This is the actual frozen publisher with this run's full admitted allocation.
            with worker.CaptureDirectory(directory, allocation) as owner:
                ref = owner.publish('preflight', 0,
                    observation(run.peers, allocation.policy, 100 + 10 * run_index, True), probe._Deadline(5))
                events.extend(({'event': 'resource_preflight', 'sequence': 0, 'outcome': 'complete',
                                'manifest': asdict(ref), 'sampling': geometry.sampling()},
                               {'event': 'clock_started', 'initial_offset_ns': -(geometry.warmup_ns + geometry.drain_ns + geometry.preparation_ahead_ns)}))
                for sample in range(geometry.samples):
                    scheduled = sample * geometry.interval_ns
                    ref = owner.publish('sample', sample + 1,
                        observation(run.peers, allocation.policy, sample + 10 * run_index), probe._Deadline(5))
                    events.extend(({'event': 'resource_request', 'kind': 'sample', 'sequence': sample + 1,
                                    'scheduled_offset_ns': scheduled, 'start_offset_ns': scheduled},
                                   {'event': 'resource_observation', 'sequence': sample + 1,
                                    'scheduled_offset_ns': scheduled, 'start_offset_ns': scheduled,
                                    'end_offset_ns': scheduled + 3_000_000, 'outcome': 'complete', 'manifest': asdict(ref)}))
            end = geometry.final + 3_000_000
            events.extend(({'event': 'resource_request', 'kind': 'finish', 'sequence': geometry.samples + 1, 'start_offset_ns': end},
                {'event': 'resource_collection_finished', 'sequence': geometry.samples + 1, 'start_offset_ns': end,
                 'end_offset_ns': end + 1_000_000, 'sampling': geometry.sampling()},
                {'event': 'request_final', 'plan': {'cohort': 'measurement', 'sequence': 1, 'logical_id': 'b' * 64,
                 'scheduled_offset_ns': 0, 'account_index': 0}, 'hash': 'c' * 63 + '1', 'offer_offset_ns': 0,
                 'acknowledgment_offset_ns': 1, 'applied_offset_ns': geometry.final, 'block_height': 1,
                 'status_attempts': 1, 'submission_finished': True, 'failure': None},
                {'event': 'collection_finished', 'passed': True, 'failure': None}))
            events = add_retention(events)
            self.events.append(events)
            journals[allocation.journal.label] = b''.join(frozen.encode(row) + b'\n' for row in events)
        controls = []
        for item in (*self.budget.static_files, *self.budget.control_budgets,
                     *(item for row in self.budget.runs for item in row.files)):
            if item.label == 'report': continue
            raw = b'x' * item.size_bytes if type(item) is budget.StaticFile else journals.get(item.label, b'x')
            path = self.root / 'controls' / item.label
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(raw)
            path.chmod(0o600)
            controls.append(bundle.ControlBinding(item.label, str(path.relative_to(self.root)), frozen.sha(raw)))
        self.controls = tuple(controls)

    def captures(self, index):
        pair, variant = RUNS[index]
        return self.root / 'resources' / f'pair-{pair:02}' / variant

    def member(self, index=0, name='sample-0000000001-peer-0000-status.body'):
        return self.captures(index) / name

    def journal(self, index):
        run = self.budget.runs[index]
        binding = next(row for row in self.controls if row.label == run.collector_journal.label)
        return self.root / binding.path

    def save_journal(self, index):
        raw = b''.join(frozen.encode(row) + b'\n' for row in self.events[index])
        self.journal(index).write_bytes(raw)
        label = self.budget.runs[index].collector_journal.label
        self.controls = tuple(replace(item, sha256=frozen.sha(raw)) if item.label == label else item for item in self.controls)

    def owner(self, **changes):
        values = dict(root=self.root, budget=self.budget, controls=self.controls, runs=self.runs,
                      expected_executable_sha256='a' * 64, reported=False)
        values.update(changes)
        return experiment.ResourceExperiment(**values)


@pytest.fixture(scope='session')
def published(tmp_path_factory):
    return PublishedExperiment(tmp_path_factory.mktemp('publisher') / 'bundle')


@pytest.fixture
def valid(published, tmp_path):
    value = copy.deepcopy(published)
    value.root = (tmp_path / 'bundle').resolve()
    shutil.copytree(published.root, value.root)
    return value


def test_all_ten_actual_publishers_and_raw_replays_link_exact_allocations_and_maxima(valid, monkeypatch):
    calls = []
    actual = experiment.replay
    def observed(*args, **kwargs):
        calls.append((args, kwargs))
        return actual(*args, **kwargs)
    monkeypatch.setattr(experiment, 'replay', observed)
    with valid.owner() as owner:
        results = owner.collect_replay()
        assert tuple((row.pair_index, row.variant) for row in results) == RUNS
        assert len(calls) == 10
        for index, (result, (args, kwargs)) in enumerate(zip(results, calls, strict=True)):
            allocation = budget.select_run_budget(valid.budget, *RUNS[index])
            assert args[0] == valid.captures(index) and args[1] == valid.journal(index)
            assert args[3] == valid.runs[index].peers and args[4] == valid.geometry
            assert kwargs == {'expected_policy': allocation.policy, 'allocation': allocation}
            assert result.replay.journal_sha256 == frozen.sha(valid.journal(index).read_bytes())
            assert result.replay.journal_bytes == valid.journal(index).stat().st_size
            assert result.replay.capture_file_count == allocation.member_count == 207
            assert result.replay.capture_bytes == sum(path.stat().st_size for path in valid.captures(index).iterdir())
            assert result.replay.capture_bytes == result.replay.raw_body_bytes + result.replay.manifest_bytes
            assert result.replay.admitted_experiment_resource_file_count == 2070
            assert result.replay.admitted_resource_byte_limit == allocation.resource_bytes
            assert result.replay.admitted_journal_byte_limit == allocation.journal.max_bytes
            assert result.maxima == experiment.ResourceMaxima(410 + 40 * index, 412 + 40 * index,
                                                             20406 + 40 * index, 806 + 40 * index)
            assert experiment.interval_maxima(result, 50_000_000, 53_000_000) == experiment.ResourceMaxima(
                30 + 40 * index, 32 + 40 * index, 16026 + 40 * index, 426 + 40 * index)
            assert len(result.replay.samples) == 22 and result.replay.samples[-1].scheduled_offset_ns == valid.geometry.final
        assert owner._bundle.snapshot.resource_files == sum(row.replay.capture_file_count for row in results) == 2070
        assert owner._bundle.snapshot.resource_bytes == sum(row.replay.capture_bytes for row in results)
        # Caller transaction/lane/effect checks belong here; this unit supplies no such authority.
        assert owner.verify() is results
    with pytest.raises(experiment.ExperimentError, match='experiment_closed'):
        owner.verify()


@pytest.mark.parametrize('change', ['missing', 'extra', 'duplicate', 'order', 'list', 'bool_pair', 'labels', 'peer_count',
    'duplicate_peer', 'geometry', 'hash', 'old_unsampled', 'bad_digest'])
def test_exact_trusted_ten_run_scope_rejected_before_bundle_io(valid, monkeypatch, change):
    rows = list(valid.runs)
    changes = {}
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
    elif change == 'bad_digest': changes['expected_executable_sha256'] = 'x'
    changes['runs'] = rows if change == 'list' else tuple(rows)
    def forbidden(*args, **kwargs): raise AssertionError('invalid scope must not open bundle')
    monkeypatch.setattr(experiment, 'BudgetedBundle', forbidden)
    with pytest.raises((experiment.ExperimentError, replay.ReplayError, budget.BudgetError)):
        valid.owner(**changes)


@pytest.mark.parametrize('index', range(10))
def test_any_failed_actual_run_never_promotes_the_successful_prefix(valid, index):
    valid.events[index][-1]['passed'] = False
    valid.save_journal(index)  # trusted control digest refreshed: semantic rejection still required.
    with valid.owner() as owner:
        with pytest.raises(replay.ReplayError, match='collection_failed'):
            owner.collect_replay()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'): owner.verify()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'): owner.collect_replay()
        assert owner._results == ()


@pytest.mark.parametrize('change', ['journal_pair', 'journal_variant', 'unsampled', 'missing_preflight', 'failed_finish', 'missing_last', 'identity', 'image'])
def test_complete_physical_bundle_cannot_replace_actual_replay_authority(valid, change):
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
    with valid.owner() as owner:
        with pytest.raises(replay.ReplayError): owner.collect_replay()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'): owner.verify()


@pytest.mark.parametrize('change', ['raw_missing', 'raw_extra', 'journal_missing', 'journal_digest', 'control_omit', 'report_present'])
def test_physical_linkage_rejects_missing_extra_or_unbound_files(valid, change):
    if change == 'raw_missing': valid.member().unlink()
    elif change == 'raw_extra': (valid.captures(9) / 'unsampled.json').write_bytes(b'{}')
    elif change == 'journal_missing': valid.journal(9).unlink()
    elif change == 'journal_digest': valid.journal(9).write_bytes(b'{}\n')
    elif change == 'control_omit': valid.controls = valid.controls[:-1]
    elif change == 'report_present': (valid.root / 'controls/report').write_bytes(b'x')
    with pytest.raises(bundle.BundleError): valid.owner()


def test_missing_and_repeated_replay_poison_the_context(valid):
    with valid.owner() as owner:
        with pytest.raises(experiment.ExperimentError, match='resource_replay_incomplete'): owner.verify()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'): owner.collect_replay()
    with valid.owner() as owner:
        owner.collect_replay()
        with pytest.raises(experiment.ExperimentError, match='resource_replay_already_attempted'): owner.collect_replay()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'): owner.verify()


@pytest.mark.parametrize('error', [KeyboardInterrupt, MemoryError, RuntimeError])
def test_interrupted_replay_cannot_publish_a_prefix(valid, monkeypatch, error):
    actual = experiment.replay
    calls = 0
    def interrupted(*args, **kwargs):
        nonlocal calls
        calls += 1
        if calls == 3: raise error('injected')
        return actual(*args, **kwargs)
    monkeypatch.setattr(experiment, 'replay', interrupted)
    with valid.owner() as owner:
        with pytest.raises(error): owner.collect_replay()
        assert calls == 3
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'): owner.verify()


@pytest.mark.parametrize('when', ['during', 'after'])
def test_tamper_after_a_run_was_replayed_is_caught_by_final_census(valid, monkeypatch, when):
    actual = experiment.replay
    calls = 0
    def tampered(*args, **kwargs):
        nonlocal calls
        value = actual(*args, **kwargs)
        calls += 1
        if calls == 10 and when == 'during': valid.member().write_bytes(b'{"queue_size":999}')
        return value
    monkeypatch.setattr(experiment, 'replay', tampered)
    with valid.owner() as owner:
        owner.collect_replay()
        if when == 'after': valid.member().write_bytes(b'{"queue_size":999}')
        with pytest.raises(bundle.BundleError): owner.verify()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'): owner.verify()


@pytest.mark.parametrize('field', ['runs', 'budget', 'controls', 'root', 'stage', 'executable'])
@pytest.mark.parametrize('when', ['during', 'after'])
def test_mutable_trusted_scope_cannot_drift_across_replay(valid, monkeypatch, field, when):
    actual = experiment.replay
    calls = 0
    def change(owner):
        if field == 'runs': object.__setattr__(owner._runs[-1].peers[0].identity, 'start_abstime', 999)
        elif field == 'budget': object.__setattr__(owner._budget.policy, 'status_body_bytes', 1024)
        elif field == 'controls': object.__setattr__(owner._controls[0], 'sha256', 'd' * 64)
        elif field == 'root': owner._root = owner._root.parent / 'other'
        elif field == 'stage': owner._reported = True
        else: owner._expected_executable_sha256 = 'd' * 64
    def changed(*args, **kwargs):
        nonlocal calls
        value = actual(*args, **kwargs)
        calls += 1
        if calls == 1 and when == 'during': change(owner)
        return value
    monkeypatch.setattr(experiment, 'replay', changed)
    with valid.owner() as owner:
        if when == 'during':
            with pytest.raises((experiment.ExperimentError, budget.BudgetError)): owner.collect_replay()
            assert calls == 1
        else:
            owner.collect_replay()
            change(owner)
            with pytest.raises((experiment.ExperimentError, budget.BudgetError)): owner.verify()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'): owner.verify()


def test_root_namespace_replacement_while_semantics_run_fails(valid):
    with valid.owner() as owner:
        owner.collect_replay()
        moved = valid.root.with_name('retained')
        valid.root.rename(moved)
        shutil.copytree(moved, valid.root)
        with pytest.raises(bundle.BundleError, match='directory_owner_changed'): owner.verify()


def test_new_reported_context_requires_fresh_ten_run_replay(valid):
    with valid.owner() as owner:
        owner.collect_replay()
        owner.verify()
    report = valid.root / 'controls/report'
    report.write_bytes(b'resource-only synthetic report')
    controls = (*valid.controls, bundle.ControlBinding('report', 'controls/report', frozen.sha(report.read_bytes())))
    with valid.owner(controls=controls, reported=True) as owner:
        with pytest.raises(experiment.ExperimentError, match='resource_replay_incomplete'): owner.verify()
    with valid.owner(controls=controls, reported=True) as owner:
        assert len(owner.collect_replay()) == 10
        assert len(owner.verify()) == 10


@pytest.mark.parametrize('start,end', [(True, 1), (-1, 1), (1, 1), (2, 1), (0, 1 << 63), (4_000_000, 5_000_000)])
def test_invalid_or_empty_observed_window_cannot_invent_maxima(valid, start, end):
    with valid.owner() as owner:
        result = owner.collect_replay()[0]
        with pytest.raises(experiment.ExperimentError): experiment.interval_maxima(result, start, end)


@pytest.mark.parametrize('field', ['warmup_ns', 'measurement_ns', 'drain_ns', 'preparation_ahead_ns',
                                  'interval_ns', 'response_deadline_ns', 'max_start_lag_ns'])
def test_every_time_scope_field_is_independent_and_consistent_before_scan(valid, monkeypatch, field):
    run = valid.runs[9]
    run = replace(run, geometry=replace(run.geometry, **{field: getattr(run.geometry, field) + 1_000_000}))
    def forbidden(*args, **kwargs): raise AssertionError('time mismatch must not open')
    monkeypatch.setattr(experiment, 'BudgetedBundle', forbidden)
    with pytest.raises((experiment.ExperimentError, replay.ReplayError, budget.BudgetError)):
        valid.owner(runs=(*valid.runs[:9], run))


def test_journal_mapping_cannot_relabel_another_fully_bound_run(valid):
    left, right = (valid.budget.runs[index].collector_journal.label for index in (0, 9))
    bindings = {item.label: item for item in valid.controls}
    valid.controls = tuple(replace(item, path=bindings[right].path, sha256=bindings[right].sha256) if item.label == left
        else replace(item, path=bindings[left].path, sha256=bindings[left].sha256) if item.label == right else item
        for item in valid.controls)
    with valid.owner() as owner:
        with pytest.raises(replay.ReplayError, match='run'): owner.collect_replay()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'): owner.verify()


@pytest.mark.parametrize('delta', [0, -1])
def test_actual_shared_journal_exact_allocation_and_one_byte_below(valid, delta):
    run = valid.budget.runs[9]
    maximum = valid.journal(9).stat().st_size + delta
    runs = (*valid.budget.runs[:9], replace(run, collector_journal=replace(run.collector_journal, max_bytes=maximum)))
    admitted = budget.admit_experiment(policy=valid.budget.policy, runs=runs, static_files=valid.budget.static_files,
        manifest=valid.budget.control_budgets[0], report=valid.budget.control_budgets[1], other_control=valid.budget.control_budgets[2:])
    if delta:
        with pytest.raises(bundle.BundleError, match='file_allocation_exceeded'): valid.owner(budget=admitted)
    else:
        with valid.owner(budget=admitted) as owner:
            rows = owner.collect_replay()
            assert rows[9].replay.admitted_journal_byte_limit == maximum == rows[9].replay.journal_bytes
            assert len(owner.verify()) == 10


def test_old_unsampled_signature_has_no_entrypoint(valid):
    with pytest.raises(TypeError):
        experiment.ResourceExperiment(valid.root, {'resource_observation': {}}, valid.controls)


def test_resource_finish_does_not_extend_transaction_drain(valid):
    rows = valid.events[9]
    final = next(row for row in rows if row['event'] == 'request_final')
    final['applied_offset_ns'] = valid.geometry.final + 1
    valid.save_journal(9)
    with valid.owner() as owner:
        with pytest.raises(replay.ReplayError, match='transaction_drain_extended'):
            owner.collect_replay()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'):
            owner.verify()
