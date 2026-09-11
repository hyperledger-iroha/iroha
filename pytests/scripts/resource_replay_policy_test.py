"""Independent rehashed capture-policy and admitted-allocation replay controls."""
from dataclasses import asdict, replace

import pytest

from resource_replay_test import (Fixture, admitted_allocation, budget, identities, metrics, ref, replay)


@pytest.fixture
def fixture(tmp_path):
    return Fixture(tmp_path)


def test_required_policy_is_present_on_actual_published_preflight_and_every_sample(fixture):
    result = fixture.run()
    for sequence in range(fixture.geometry.samples + 1):
        _, _, manifest = fixture.capture(sequence)
        assert manifest['capture_policy'] == asdict(fixture.policy)
    assert result.admitted_resource_byte_limit == fixture.allocation.resource_bytes
    assert result.capture_file_count == fixture.allocation.member_count
    assert result.capture_bytes <= fixture.allocation.resource_bytes
    assert result.journal_bytes <= fixture.allocation.journal.max_bytes


@pytest.mark.parametrize('sequence', [0, 1, 22])
@pytest.mark.parametrize('change', ['missing', 'empty', 'extra', 'bool', 'float', 'zero', 'larger', 'smaller', 'overflow'])
def test_rehashed_capture_policy_must_match_independent_policy_at_every_stage(fixture, sequence, change):
    event, path, manifest = fixture.capture(sequence)
    policy = manifest['capture_policy']
    if change == 'missing': manifest.pop('capture_policy')
    if change == 'empty': manifest['capture_policy'] = {}
    if change == 'extra': policy['old_default'] = 1
    if change == 'bool': policy['status_body_bytes'] = True
    if change == 'float': policy['status_body_bytes'] = float(fixture.policy.status_body_bytes)
    if change == 'zero': policy['metrics_body_bytes'] = 0
    if change == 'larger': policy['status_body_bytes'] += 1
    if change == 'smaller': policy['metrics_body_bytes'] -= 1
    if change == 'overflow': policy['metrics_body_bytes'] = 1 << 128
    fixture.rewrite(event, path, manifest)
    with pytest.raises(replay.ReplayError): fixture.run()


@pytest.mark.parametrize('field', ['resource_bytes_per_run', 'resource_bytes', 'members_per_run',
                                  'resource_member_count', 'bytes_per_capture', 'total_bytes'])
@pytest.mark.parametrize('value', [1, True, -1, 1 << 128])
def test_unadmitted_numerical_allocations_fail_before_any_evidence_open(fixture, monkeypatch, field, value):
    allocation = replace(fixture.allocation, experiment=replace(fixture.allocation.experiment, **{field: value}))
    def forbidden(*args): raise AssertionError('invalid budget reached filesystem')
    monkeypatch.setattr(replay, '_open_directory', forbidden)
    with pytest.raises(replay.ReplayError, match='resource_budget_invalid'):
        replay.replay(fixture.directory, fixture.journal, fixture.digest, fixture.peers, fixture.geometry,
                      expected_policy=fixture.policy, allocation=allocation)


@pytest.mark.parametrize('change', ['missing_policy', 'policy_type', 'different_policy', 'allocation_type',
                                  'peer_count', 'interval', 'measurement', 'drain'])
def test_required_trusted_inputs_and_geometry_cannot_be_inferred_from_manifests(fixture, monkeypatch, change):
    policy, allocation = fixture.policy, fixture.allocation
    if change == 'missing_policy': policy = None
    if change == 'policy_type': policy = asdict(policy)
    if change == 'different_policy': policy = replace(policy, metrics_body_bytes=policy.metrics_body_bytes - 1)
    if change == 'allocation_type': allocation = asdict(allocation)
    if change in ('peer_count', 'interval', 'measurement', 'drain'):
        peers, geometry = len(fixture.peers), fixture.geometry
        if change == 'peer_count': peers = 5
        if change == 'interval':
            # Keep this alternate cadence independently admissible: doubling all
            # resource durations preserves22 samples and isolates replay's binding.
            geometry = replace(geometry, interval_ns=20_000_000,
                               measurement_ns=400_000_000, drain_ns=20_000_000)
        if change == 'measurement': geometry = replace(geometry, measurement_ns=210_000_000)
        if change == 'drain': geometry = replace(geometry, drain_ns=20_000_000)
        allocation = admitted_allocation(peers, geometry, policy)
    def forbidden(*args): raise AssertionError('mismatched trusted inputs reached filesystem')
    monkeypatch.setattr(replay, '_open_directory', forbidden)
    with pytest.raises(replay.ReplayError):
        replay.replay(fixture.directory, fixture.journal, fixture.digest, fixture.peers, fixture.geometry,
                      expected_policy=policy, allocation=allocation)


def test_omitting_new_required_arguments_is_not_an_old_unsampled_acceptor(fixture):
    with pytest.raises(TypeError):
        replay.replay(fixture.directory, fixture.journal, fixture.digest, fixture.peers, fixture.geometry)


@pytest.mark.parametrize('field,value', [('pair_index', 2), ('pair_index', True), ('variant', 'four_lane')])
def test_journal_plan_must_identify_the_exact_allocated_run(fixture, field, value):
    fixture.events[0][field] = value
    with pytest.raises(replay.ReplayError, match='journal_run_budget_mismatch'): fixture.run()


def test_other_valid_run_budget_cannot_relabel_same_journal(fixture):
    fixture.allocation = budget.select_run_budget(fixture.allocation.experiment, 2, 'four_lane')
    with pytest.raises(replay.ReplayError, match='journal_run_budget_mismatch'): fixture.run()


def test_exact_journal_allocation_succeeds_and_one_byte_less_fails_before_journal_read(fixture, monkeypatch):
    fixture.save()
    size = fixture.journal.stat().st_size
    fixture.allocation = admitted_allocation(len(fixture.peers), fixture.geometry, fixture.policy, size)
    assert fixture.run().journal_bytes == size
    fixture.allocation = admitted_allocation(len(fixture.peers), fixture.geometry, fixture.policy, size - 1)
    original = replay.os.pread
    def forbidden_journal(fd, count, offset):
        # Journal admission occurs before any body/manifest read, so no pread is valid.
        raise AssertionError('underallocated journal was read')
    monkeypatch.setattr(replay.os, 'pread', forbidden_journal)
    with pytest.raises(replay.ReplayError, match='journal_allocation_exceeded'): fixture.run()
    monkeypatch.setattr(replay.os, 'pread', original)


@pytest.mark.parametrize('role', ['status', 'metrics'])
@pytest.mark.parametrize('sequence', [0, 22])
@pytest.mark.parametrize('delta', [0, 1])
def test_matching_policy_header_enforces_exact_rehashed_raw_body_cap(fixture, monkeypatch, role, sequence, delta):
    event, path, manifest = fixture.capture(sequence)
    owner = manifest['peers'][0][role]
    body_path = fixture.directory / owner['body']['name']
    before = body_path.read_bytes()
    limit = fixture.policy.status_body_bytes if role == 'status' else fixture.policy.metrics_body_bytes
    if role == 'status':
        prefix, suffix = b'{"queue_size":1,"padding":"', b'"}'
        raw = prefix + b'x' * (limit + delta - len(prefix) - len(suffix)) + suffix
    else:
        remaining = limit + delta - len(before)
        line = b'#' + b'x' * 1022 + b'\n'
        tail = remaining % len(line)
        raw = before + line * (remaining // len(line))
        raw += b'\n' if tail == 1 else b'#' + b'x' * (tail - 2) + b'\n' if tail else b''
        owner['kura'] = asdict(metrics.parse_kura_resource_metrics(raw))
    assert len(raw) == limit + delta
    assert manifest['capture_policy'] == asdict(fixture.policy)
    body_path.write_bytes(raw)
    owner['body'] = ref(body_path)
    manifest['wire_bytes'] += len(raw) - len(before)
    fixture.rewrite(event, path, manifest)
    opened = []
    original = replay.os.open
    def tracked(name, *args, **kwargs):
        if name == body_path.name: opened.append(name)
        return original(name, *args, **kwargs)
    monkeypatch.setattr(replay.os, 'open', tracked)
    if delta:
        with pytest.raises(replay.ReplayError, match='integer_outside_bounds'): fixture.run()
        assert not opened
    else:
        result = fixture.run()
        assert result.capture_bytes <= result.admitted_resource_byte_limit
        assert opened == [body_path.name]


def test_full_experiment_global_overallocation_cannot_be_hidden_by_one_small_run(fixture, monkeypatch):
    experiment = fixture.allocation.experiment
    runs = tuple(replace(run, collector_journal=replace(run.collector_journal, max_bytes=budget.MAX_FILE_BYTES))
                 for run in experiment.runs)
    fixture.allocation = replace(fixture.allocation, experiment=replace(experiment, runs=runs))
    def forbidden(*args): raise AssertionError('globally impossible experiment reached filesystem')
    monkeypatch.setattr(replay, '_open_directory', forbidden)
    with pytest.raises(replay.ReplayError, match='resource_budget_invalid'): fixture.run()


def scope():
    geometry = replay.ReplayGeometry(100_000_000, 200_000_000, 10_000_000, 20_000_000,
                                     10_000_000, 4_000_000, 2_000_000)
    policy = budget.CapturePolicy()
    return identities(), geometry, policy, admitted_allocation(4, geometry, policy)


def test_public_scope_owner_is_pure_and_returns_full_readmitted_allocation(monkeypatch):
    peers, geometry, policy, allocation = scope()
    def forbidden(*args, **kwargs): raise AssertionError('pure scope opened evidence')
    monkeypatch.setattr(replay, '_open_directory', forbidden)
    monkeypatch.setattr(replay.os, 'open', forbidden)
    result = replay.validate_replay_scope(peers, geometry, expected_policy=policy, allocation=allocation)
    assert result == allocation
    assert result.experiment.total_bytes <= budget.MAX_TOTAL_BYTES
    assert result.journal == allocation.run.collector_journal


@pytest.mark.parametrize('change', ['peer_list', 'duplicate_peer', 'lifetime', 'geometry_type',
    'geometry_cadence', 'policy_type', 'policy_mismatch', 'allocation_type', 'summary', 'missing_run', 'peer_geometry'])
def test_public_scope_rejects_invalid_inputs_without_filesystem_admission(monkeypatch, change):
    peers, geometry, policy, allocation = scope()
    if change == 'peer_list': peers = list(peers)
    if change == 'duplicate_peer': peers = (peers[0], peers[0], *peers[2:])
    if change == 'lifetime': peers = (replace(peers[0], identity=replace(peers[0].identity, image_uuid='0'*32)), *peers[1:])
    if change == 'geometry_type': geometry = asdict(geometry)
    if change == 'geometry_cadence': geometry = replace(geometry, interval_ns=0)
    if change == 'policy_type': policy = asdict(policy)
    if change == 'policy_mismatch': policy = replace(policy, metrics_body_bytes=policy.metrics_body_bytes-1)
    if change == 'allocation_type': allocation = asdict(allocation)
    if change == 'summary': allocation = replace(allocation, experiment=replace(allocation.experiment, total_bytes=True))
    if change == 'missing_run': allocation = replace(allocation, experiment=replace(allocation.experiment, runs=allocation.experiment.runs[:-1]))
    if change == 'peer_geometry': allocation = admitted_allocation(5, geometry, policy)
    def forbidden(*args, **kwargs): raise AssertionError('invalid scope reached evidence')
    monkeypatch.setattr(replay, '_open_directory', forbidden)
    monkeypatch.setattr(replay.os, 'open', forbidden)
    with pytest.raises(replay.ReplayError):
        replay.validate_replay_scope(peers, geometry, expected_policy=policy, allocation=allocation)


def test_replay_calls_public_scope_owner_before_any_file_input(fixture, monkeypatch):
    reached = []
    def rejected(*args, **kwargs):
        reached.append((args, kwargs))
        raise replay.ReplayError('independent_scope_rejected')
    monkeypatch.setattr(replay, 'validate_replay_scope', rejected)
    def forbidden(*args, **kwargs): raise AssertionError('scope rejection reached evidence')
    monkeypatch.setattr(replay, '_open_directory', forbidden)
    with pytest.raises(replay.ReplayError, match='independent_scope_rejected'):
        replay.replay(fixture.directory, fixture.journal, 'not-a-digest', fixture.peers, fixture.geometry,
                      expected_policy=fixture.policy, allocation=fixture.allocation)
    assert len(reached) == 1
    assert reached[0] == ((fixture.peers, fixture.geometry),
                           {'expected_policy': fixture.policy, 'allocation': fixture.allocation})
