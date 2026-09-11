"""Synthetic worker/immutable-file tests; no validator or native process sampling."""
from contextlib import ExitStack
from dataclasses import asdict, replace
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/nexus'))
import resource_probe_worker as worker
import resource_probe as probe
import resource_process as process
import kura_resource_metrics as metrics
from resource_evidence_budget import (
    CaptureGeometry, CapturePolicy, FileBudget, RunBudget, StaticFile, admit_experiment,
    run_budget_inputs, select_run_budget, run_budget_sha256, canonical_run_budget_bytes,
)

POLICY = CapturePolicy(status_body_bytes=4096, metrics_body_bytes=16 * 1024)


def allocation(count=4, policy=POLICY):
    geometry = CaptureGeometry(count, 2_000_000, 40_000_000, 2_000_000)
    runs = tuple(RunBudget(pair, variant, geometry,
                           *(FileBudget(f'pair{pair}.{variant}.{role}', 4096)
                             for role in ('journal', 'trace', 'proof', 'log', 'raw')),
                           support=())
                 for pair in range(1, 6) for variant in ('one_lane', 'four_lane'))
    experiment = admit_experiment(policy=policy, runs=runs,
                                 static_files=(StaticFile('worker_fixture', Path(__file__).stat().st_size),),
                                 manifest=FileBudget('manifest', 4096), report=FileBudget('report', 4096),
                                 other_control=())
    return select_run_budget(experiment, 1, 'one_lane')


def admission_reply(admitted):
    """Independently assemble the exact public receipt expected at the CLI boundary."""
    return {'schema': worker.RESPONSE_SCHEMA, 'kind': 'admit', 'sequence': 0,
            'outcome': 'complete', 'admission': {
                'schema': worker.ADMISSION_SCHEMA, 'budget_sha256': run_budget_sha256(admitted),
                'pair_index': admitted.run.pair_index, 'variant': admitted.run.variant,
                'geometry': asdict(admitted.geometry), 'journal': asdict(admitted.journal),
                'trace': asdict(admitted.run.transaction_trace)}}


def configured(result=None, collect=None):
    result = result or observation()
    return worker.ConfiguredProbe(SimpleNamespace(collect=collect or (lambda _: result)),
                                  allocation(len(result.peers), result.capture_policy))


def observation(count=4, available=True):
    rows = ['iroha_kura_resource_available 1', 'iroha_kura_resource_status{reason="available"} 1',
            'iroha_kura_resource_generation 1', 'iroha_kura_resource_fault_count 0']
    fields = ('resident_associations', 'persisted_entries', 'index_bytes', 'temporary_index_bytes', 'storage_bytes')
    for family in metrics.FAMILIES:
        rows.extend(f'iroha_kura_resource_{field}{{family="{family}"}} 0' for field in fields)
    rows.extend(f'iroha_kura_resource_{field}_sum 0' for field in fields)
    rows.append('iroha_kura_resource_represented_entries 0')
    raw = ('\n'.join(rows) + '\n').encode() if available else (
        b'iroha_kura_resource_available 0\niroha_kura_resource_status{reason="busy"} 1\n')
    kura = metrics.parse_kura_resource_metrics(raw)
    status = b'{"queue_size":5,"other":"public diagnostics"}'
    def body(route, raw, content_type):
        return probe.HttpProvenance(route, hashlib.sha256(raw).hexdigest(), len(raw), content_type, raw)
    peers = []
    for index in range(count):
        sample = process.ProcessSample(process.ProcessIdentity(100 + index, 501, 1, 0, 1,
                                       '01' * 16, 'a' * 64), 1024)
        peers.append(probe.PeerObservation(f'peer{index}', 5, body('/status', status, 'application/json'),
                                          body('/metrics', raw, 'text/plain'), kura, sample, sample))
    return probe.ProbeObservation(tuple(peers), 1000000, count * (len(raw) + len(status) + 100),
                                  count * 1024, count * 1024, count * 5, 5,
                                  probe.InventoryAggregate(0, 0) if available else None, POLICY)


def request(kind, sequence, **extra):
    return {**{'schema': worker.REQUEST_SCHEMA, 'kind': kind, 'sequence': sequence,
               'timeout_ms': 1000}, **extra}


def encoded(value):
    return json.dumps(value, separators=(',', ':')).encode() + b'\n'


def capture_dir(tmp_path):
    path = tmp_path / 'captures'
    path.mkdir(mode=0o700)
    return path


def execute(tmp_path, requests, result=None, builder=None):
    path = capture_dir(tmp_path)
    output = io.BytesIO()
    result = result or observation()
    factory = builder or (lambda _path, _owners: configured(result))
    code = worker.run_worker(tmp_path / 'runtime-secret-config.json', path,
                             io.BytesIO(encoded(request('admit', 0)) + b''.join(encoded(item) for item in requests)), output,
                             probe_builder=factory)
    responses = [json.loads(line) for line in output.getvalue().splitlines()]
    assert responses[0] == admission_reply(allocation(len(result.peers), result.capture_policy))
    return code, responses[1:], path


def execute_admission_failure(tmp_path, builder):
    """An existing directory remains untouched; failed admit permits only finish0."""
    path = capture_dir(tmp_path)
    output = io.BytesIO()
    code = worker.run_worker(tmp_path / 'runtime-secret-config.json', path,
                             io.BytesIO(encoded(request('admit', 0)) + encoded(request('finish', 0))),
                             output, probe_builder=builder)
    return code, [json.loads(line) for line in output.getvalue().splitlines()], path


def validate_reference(path, reference):
    raw = (path / reference['name']).read_bytes()
    assert len(raw) == reference['bytes']
    assert hashlib.sha256(raw).hexdigest() == reference['sha256']
    info = (path / reference['name']).stat()
    assert stat.S_ISREG(info.st_mode)
    assert stat.S_IMODE(info.st_mode) == 0o600
    assert info.st_nlink == 1
    return raw


def test_complete_persistent_session_publishes_exact_raw_files_and_one_manifest_reference(tmp_path):
    result = observation()
    code, responses, path = execute(tmp_path, [request('preflight', 0), request('sample', 1),
                                               request('sample', 2), request('finish', 3)], result)
    assert code == 0
    assert [row['sequence'] for row in responses] == [0, 1, 2, 3]
    assert all(row['outcome'] == 'complete' for row in responses)
    assert responses[-1]['manifest'] is None
    assert len(list(path.iterdir())) == 3 * 9
    for index, response in enumerate(responses[:-1]):
        assert len(encoded(response)) <= worker.MAX_FRAME_BYTES
        raw = validate_reference(path, response['manifest'])
        manifest = json.loads(raw)
        assert manifest['schema'] == worker.CAPTURE_SCHEMA
        assert manifest['kind'] == response['kind']
        assert manifest['sequence'] == index
        assert manifest['capture_policy'] == {'status_body_bytes': POLICY.status_body_bytes,
                                             'metrics_body_bytes': POLICY.metrics_body_bytes}
        assert manifest['available'] is True
        assert manifest['aggregates']['rss_before_bytes'] == 4096
        assert len(manifest['peers']) == 4
        for peer, expected in zip(manifest['peers'], result.peers, strict=True):
            assert validate_reference(path, peer['status']['body']) == expected.status.raw_body
            assert validate_reference(path, peer['metrics']['body']) == expected.metrics.raw_body
            assert metrics.parse_kura_resource_metrics(expected.metrics.raw_body).raw_sha256 == peer['metrics']['kura']['raw_sha256']
        for forbidden in ('endpoint', 'headers', 'executable_path', 'timestamp_ms', 'request_start_ns'):
            assert forbidden not in manifest
            assert forbidden.encode() not in raw
    assert stat.S_IMODE(path.stat().st_mode) == 0o700


def test_unavailable_is_replayable_without_inventory_and_cannot_admit_a_sample(tmp_path):
    code, responses, path = execute(tmp_path, [request('preflight', 0), request('sample', 1)], observation(available=False))
    assert code == 1
    assert [response['outcome'] for response in responses] == ['unavailable', 'failed']
    manifest = json.loads(validate_reference(path, responses[0]['manifest']))
    assert manifest['available'] is False
    assert manifest['aggregates']['inventory'] is None
    assert len(manifest['peers']) == 4
    for peer in manifest['peers']:
        assert peer['metrics']['kura']['reason'] == 'busy'
        assert 'generation' not in peer['metrics']['kura']
        assert metrics.parse_kura_resource_metrics(validate_reference(path, peer['metrics']['body'])).available is False
    assert responses[1]['manifest'] is None


def test_all64_peers_fit_single_manifest_and_bounded_ipc(tmp_path):
    code, responses, path = execute(tmp_path, [request('preflight', 0), request('finish', 1)], observation(64))
    assert code == 0
    assert len(encoded(responses[0])) < 1024
    raw = validate_reference(path, responses[0]['manifest'])
    assert len(raw) <= worker.MAX_MANIFEST_BYTES
    assert len(json.loads(raw)['peers']) == 64
    assert len(list(path.iterdir())) == 129


@pytest.mark.parametrize('requests', [
    [request('sample', 0)], [request('finish', 0)], [request('preflight', 1)],
    [request('preflight', 0), request('preflight', 1)],
    [request('preflight', 0), request('sample', 0)],
    [request('preflight', 0), request('sample', 2)],
    [request('preflight', 0), request('finish', 2)],
])
def test_sequence_reordering_replay_and_missing_preflight_fail_closed(tmp_path, requests):
    code, responses, _ = execute(tmp_path, requests)
    assert code == 1
    assert responses[-1]['outcome'] == 'failed'
    assert responses[-1]['manifest'] is None


@pytest.mark.parametrize('mutation', ['extra', 'schema', 'kind', 'bool_sequence', 'negative_sequence', 'late_sequence',
                                      'bool_timeout', 'zero_timeout', 'large_timeout', 'float', 'nested'])
def test_request_shape_and_numeric_bounds(mutation):
    row = request('preflight', 0)
    if mutation == 'extra': row['private'] = 'never-reflect'
    if mutation == 'schema': row['schema'] += '.legacy'
    if mutation == 'kind': row['kind'] = 'secret'
    if mutation == 'bool_sequence': row['sequence'] = True
    if mutation == 'negative_sequence': row['sequence'] = -1
    if mutation == 'late_sequence': row['sequence'] = 100002
    if mutation == 'bool_timeout': row['timeout_ms'] = True
    if mutation == 'zero_timeout': row['timeout_ms'] = 0
    if mutation == 'large_timeout': row['timeout_ms'] = 60001
    if mutation == 'float': row['sequence'] = 1.0
    if mutation == 'nested': row['kind'] = {'secret': 'never-reflect'}
    with pytest.raises(probe.ProbeError):
        worker._request(encoded(row))


@pytest.mark.parametrize('raw', [
    b'', b'{}', b'{}\n{}\n', b' ' * 16385, b'{"sequence":0,"sequence":0}\n',
    b'[' * 33 + b']' * 33 + b'\n', b'\xff\n', b'{"x":NaN}\n',
    b'{"x":' + b'9' * 33 + b'}\n',
])
def test_malformed_frames_and_json_are_bounded(raw):
    with pytest.raises(probe.ProbeError):
        worker._request(raw)


def test_empty_input_missing_finish_and_oversized_input_never_succeed(tmp_path):
    path = capture_dir(tmp_path)
    for raw in (b'', b' ' * (worker.MAX_FRAME_BYTES + 2)):
        output = io.BytesIO()
        assert worker.run_worker(tmp_path / 'unused', path, io.BytesIO(raw), output,
                                  probe_builder=lambda *_: None) == 1
        assert output.getvalue() == b''
    other = tmp_path / 'other'
    other.mkdir()
    code, responses, _ = execute(other, [request('preflight', 0)])
    assert code == 1
    assert responses[0]['outcome'] == 'complete'


def test_config_failure_is_static_and_allows_explicit_clean_finish(tmp_path):
    def failed_builder(*_):
        raise OSError('Authorization: Bearer secret-runtime-token /private/path')
    code, responses, path = execute_admission_failure(tmp_path, failed_builder)
    assert code == 0
    assert [row['outcome'] for row in responses] == ['failed', 'complete']
    assert all(row['admission' if row['kind'] == 'admit' else 'manifest'] is None for row in responses)
    assert b'secret' not in encoded(responses)
    assert list(path.iterdir()) == []


def test_no_clobber_body_collision_preserves_original_and_rejects_manifest(tmp_path):
    path = capture_dir(tmp_path)
    sentinel = b'preexisting exact bytes'
    name = 'preflight-0000000000-peer-0000-status.body'
    def collect(_):
        (path / name).write_bytes(sentinel)
        return observation()
    def build(*_):
        return configured(collect=collect)
    output = io.BytesIO()
    code = worker.run_worker(tmp_path / 'unused', path,
                             io.BytesIO(encoded(request('admit', 0)) + encoded(request('preflight', 0)) + encoded(request('finish', 1))),
                             output, probe_builder=build)
    assert code == 0
    assert json.loads(output.getvalue().splitlines()[1])['outcome'] == 'failed'
    assert (path / name).read_bytes() == sentinel
    assert not (path / 'preflight-0000000000.json').exists()


@pytest.mark.parametrize('fault', ['fsync', 'write', 'readback', 'manifest_cap'])
def test_capture_failure_retains_partial_files_without_qualifying_reference(tmp_path, monkeypatch, fault):
    if fault == 'fsync':
        monkeypatch.setattr(worker.os, 'fsync', lambda _: (_ for _ in ()).throw(OSError('secret')))
    if fault == 'write':
        monkeypatch.setattr(worker.os, 'write', lambda *_: 0)
    if fault == 'readback':
        monkeypatch.setattr(worker.os, 'pread', lambda *_: b'wrong')
    if fault == 'manifest_cap':
        monkeypatch.setattr(worker, 'MAX_MANIFEST_BYTES', 32)
    code, responses, path = execute(tmp_path, [request('preflight', 0), request('finish', 1)])
    assert code == 0
    assert responses[0]['outcome'] == 'failed'
    assert responses[0]['manifest'] is None
    if fault == 'manifest_cap':
        # Full reservation admission now rejects an oversized manifest before
        # the first create. Actual write/sync/readback failures retain artifacts.
        assert len(list(path.iterdir())) == 0
    else:
        assert len(list(path.iterdir())) >= 1
    assert not (path / 'preflight-0000000000.json').exists()


def test_capture_timeout_covers_flush_and_publication(tmp_path, monkeypatch):
    clock = [10_000_000_000]
    monkeypatch.setattr(probe.time, 'monotonic_ns', lambda: clock[0])
    original = worker.os.fsync
    def slow_sync(fd):
        original(fd)
        clock[0] += 2_000_000_000
    monkeypatch.setattr(worker.os, 'fsync', slow_sync)
    code, responses, path = execute(tmp_path, [request('preflight', 0), request('finish', 1)])
    assert code == 0
    assert responses[0]['outcome'] == 'failed'
    assert responses[0]['manifest'] is None
    assert not (path / 'preflight-0000000000.json').exists()


@pytest.mark.parametrize('mutation', ['mode', 'nonempty', 'symlink', 'file'])
def test_capture_directory_requires_new_exact_owner(tmp_path, mutation):
    path = tmp_path / 'captures'
    if mutation == 'file':
        path.write_bytes(b'file')
    elif mutation == 'symlink':
        target = tmp_path / 'actual'
        target.mkdir(mode=0o700)
        path.symlink_to(target)
    else:
        path.mkdir(mode=0o700)
        if mutation == 'mode': path.chmod(0o755)
        if mutation == 'nonempty': (path / 'prior').write_bytes(b'old')
    with pytest.raises((probe.ProbeError, OSError)):
        worker.CaptureDirectory(path, allocation())


def test_directory_replacement_and_file_symlink_cannot_redirect_write(tmp_path):
    path = capture_dir(tmp_path)
    with worker.CaptureDirectory(path, allocation()) as directory:
        retired = tmp_path / 'retired'
        path.rename(retired)
        path.mkdir(mode=0o700)
        with pytest.raises(probe.ProbeError, match='capture_directory_changed'):
            directory.publish('preflight', 0, observation(), probe._Deadline(1))
        assert list(path.iterdir()) == []
    other = tmp_path / 'second'
    other.mkdir(mode=0o700)
    external = tmp_path / 'external'
    external.write_bytes(b'exact')
    with worker.CaptureDirectory(other, allocation()) as directory:
        (other / 'preflight-0000000000.json').symlink_to(external)
        with pytest.raises(FileExistsError):
            directory.publish('preflight', 0, observation(), probe._Deadline(1))
    assert external.read_bytes() == b'exact'


def config():
    return {'schema': worker.CONFIG_SCHEMA, 'peers': [
        {'peer_id': f'peer{i}', 'pid': 100 + i, 'executable_path': '/absolute/iroha3d',
         'executable_sha256': 'a' * 64, 'endpoint': f'http://127.0.0.1:{8000+i}',
         'headers': {'authorization': 'Bearer runtime-only-secret'}} for i in range(4)],
            'resource_budget': run_budget_inputs(allocation())}


def write_config(tmp_path, value):
    path = tmp_path / 'runtime.json'
    path.write_bytes(encoded(value))
    path.chmod(0o600)
    return path


def test_config_is_exact_runtime_only_and_never_exported(tmp_path):
    path = write_config(tmp_path, config())
    assert worker.read_config(path) == config()
    assert stat.S_IMODE(path.stat().st_mode) == 0o600


@pytest.mark.parametrize('mutation', ['public', 'symlink', 'empty', 'large', 'extra', 'three', 'duplicate_pid',
                                      'duplicate_peer', 'relative_image', 'bad_hash', 'url_secret', 'bad_header'])
def test_config_rejects_unsafe_or_incomplete_prelaunch_inputs(tmp_path, mutation):
    value = config()
    if mutation == 'extra': value['legacy'] = True
    if mutation == 'three': value['peers'].pop()
    if mutation == 'duplicate_pid': value['peers'][1]['pid'] = value['peers'][0]['pid']
    if mutation == 'duplicate_peer': value['peers'][1]['peer_id'] = value['peers'][0]['peer_id']
    if mutation == 'relative_image': value['peers'][0]['executable_path'] = 'relative'
    if mutation == 'bad_hash': value['peers'][0]['executable_sha256'] = 'A' * 64
    if mutation == 'url_secret': value['peers'][0]['endpoint'] += '?token=secret'
    if mutation == 'bad_header': value['peers'][0]['headers'] = {'Host': 'secret'}
    path = write_config(tmp_path, value)
    if mutation == 'public': path.chmod(0o644)
    if mutation == 'empty': path.write_bytes(b'')
    if mutation == 'large': path.write_bytes(b' ' * (worker.MAX_CONFIG_BYTES + 1))
    if mutation == 'symlink':
        target = tmp_path / 'other'
        path.rename(target)
        path.symlink_to(target)
    with pytest.raises((probe.ProbeError, OSError)):
        worker.read_config(path)


def test_exact_config_file_identity_is_rechecked_after_read(tmp_path, monkeypatch):
    path = write_config(tmp_path, config())
    original = worker.os.pread
    def replaced(fd, count, offset):
        raw = original(fd, count, offset)
        path.unlink()
        path.write_bytes(raw)
        path.chmod(0o600)
        return raw
    monkeypatch.setattr(worker.os, 'pread', replaced)
    with pytest.raises(probe.ProbeError, match='config_file_changed'):
        worker.read_config(path)


def test_build_probe_reuses_hashed_image_and_closes_it_without_discovery(tmp_path, monkeypatch):
    path = write_config(tmp_path, config())
    opened, closed, samples = [], [], []
    class Image:
        def __init__(self, path, digest): opened.append((path, digest))
        def __enter__(self): return self
        def __exit__(self, *_): closed.append(True)
    class Reader:
        def sample(self, pid, image):
            samples.append(pid)
            identity = process.ProcessIdentity(pid, 501, 1, 0, 1, '01'*16, 'a'*64)
            return process.ProcessSample(identity, 1024)
    monkeypatch.setattr(worker, 'ExecutableImage', Image)
    monkeypatch.setattr(worker, 'DarwinProcessReader', Reader)
    with ExitStack() as owners:
        result = worker.build_probe(path, owners)
        assert len(result.collector.peers) == 4
        assert result.allocation.policy == POLICY
        assert len(opened) == 1
        assert samples == [100, 101, 102, 103]
        assert closed == []
    assert closed == [True]


def test_actual_persistent_python_pipe_uses_same_worker_state_machine(tmp_path):
    path = capture_dir(tmp_path)
    harness = tmp_path / 'synthetic_worker.py'
    harness.write_text('import sys\nfrom pathlib import Path\n'
                      f'sys.path.insert(0, {str(Path(__file__).parent)!r})\n'
                      'from resource_probe_worker_test import worker, configured\n'
                      f'raise SystemExit(worker.run_worker(Path("/unused"), Path({str(path)!r}), '
                      'sys.stdin.buffer, sys.stdout.buffer, '
                      'probe_builder=lambda *_: configured()))\n')
    result = subprocess.run([sys.executable, str(harness)],
                            input=encoded(request('admit', 0))+encoded(request('preflight', 0))+encoded(request('sample', 1))+encoded(request('finish', 2)),
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5, check=False)
    assert result.returncode == 0
    assert result.stderr == b''
    all_rows = [json.loads(line) for line in result.stdout.splitlines()]
    assert all_rows[0]['kind'] == 'admit' and all_rows[0]['outcome'] == 'complete'
    rows = all_rows[1:]
    assert [row['sequence'] for row in rows] == [0, 1, 2]
    assert all(row['outcome'] == 'complete' for row in rows)
    assert len(list(path.iterdir())) == 18


def test_main_bad_arguments_do_not_echo_paths_or_secrets(capsys):
    assert worker.main(['--private-token', 'NeverPrintMe']) == 2
    assert capsys.readouterr() == ('', '')


def test_capture_hardlink_cannot_create_another_mutable_owner(tmp_path, monkeypatch):
    path = capture_dir(tmp_path)
    original = worker.os.fsync
    linked = []
    def link_before_validation(fd):
        if not linked and stat.S_ISREG(os.fstat(fd).st_mode):
            name = 'preflight-0000000000-peer-0000-status.body'
            os.link(path / name, tmp_path / 'another-owner')
            linked.append(True)
        original(fd)
    monkeypatch.setattr(worker.os, 'fsync', link_before_validation)
    output = io.BytesIO()
    worker.run_worker(tmp_path/'unused', path,
                      io.BytesIO(encoded(request('admit', 0))+encoded(request('preflight', 0))+encoded(request('finish', 1))), output,
                      probe_builder=lambda *_: configured())
    assert linked == [True]
    assert json.loads(output.getvalue().splitlines()[1])['outcome'] == 'failed'
    assert not (path/'preflight-0000000000.json').exists()


def test_config_hardlink_and_duplicate_endpoint_are_rejected(tmp_path):
    path = write_config(tmp_path, config())
    os.link(path, tmp_path/'second-name')
    with pytest.raises(probe.ProbeError, match='config_file_invalid'):
        worker.read_config(path)
    (tmp_path/'second-name').unlink()
    value = config()
    value['peers'][1]['endpoint'] = value['peers'][0]['endpoint']
    path.write_bytes(encoded(value))
    with pytest.raises(probe.ProbeError, match='config_peer_invalid'):
        worker.read_config(path)


@pytest.mark.parametrize('kind,sequence', [('preflight',1),('sample',0),('sample',100001),('finish',1)])
def test_publisher_cannot_construct_out_of_protocol_capture_names(tmp_path, kind, sequence):
    with worker.CaptureDirectory(capture_dir(tmp_path), allocation()) as directory:
        with pytest.raises(probe.ProbeError, match='capture_sequence_invalid'):
            directory.publish(kind, sequence, observation(), probe._Deadline(1))


def test_expired_config_initialization_does_not_start_http(tmp_path, monkeypatch):
    clock = [10_000_000_000]
    monkeypatch.setattr(probe.time, 'monotonic_ns', lambda: clock[0])
    calls = []
    def build(*_):
        clock[0] += 2_000_000_000
        return configured(collect=lambda _: calls.append('http'))
    code, responses, path = execute_admission_failure(tmp_path, build)
    assert code == 0
    assert calls == []
    assert responses[0]['outcome'] == 'failed'
    assert list(path.iterdir()) == []


def test_manifest_collision_never_replaces_prior_evidence(tmp_path):
    path = capture_dir(tmp_path)
    original = b'prior immutable manifest'
    name = 'preflight-0000000000.json'
    def collect(_):
        (path/name).write_bytes(original)
        return observation()
    def build(*_):
        return configured(collect=collect)
    output = io.BytesIO()
    worker.run_worker(tmp_path/'unused', path,
                      io.BytesIO(encoded(request('admit',0))+encoded(request('preflight',0))+encoded(request('finish',1))), output,
                      probe_builder=build)
    assert json.loads(output.getvalue().splitlines()[1])['outcome'] == 'failed'
    assert (path/name).read_bytes() == original
    assert len(list(path.iterdir())) == 9


@pytest.mark.parametrize('mutation', [
    'old_shape', 'missing_policy', 'missing_cap', 'extra_cap', 'bool_cap', 'zero_cap',
    'status_ceiling', 'metrics_ceiling', 'oversubscribed_experiment', 'peer_geometry',
    'missing_run', 'wrong_pair', 'computed_summary', 'extra_policy',
])
def test_invalid_policy_or_full_budget_fails_before_capture_or_process_owners(tmp_path, monkeypatch, mutation):
    value = config()
    budget = value['resource_budget']
    experiment = budget['experiment']
    policy = experiment['capture_policy']
    if mutation == 'old_shape': value.pop('resource_budget')
    if mutation == 'missing_policy': experiment.pop('capture_policy')
    if mutation == 'missing_cap': policy.pop('metrics_body_bytes')
    if mutation == 'extra_cap': policy['other'] = 1
    if mutation == 'bool_cap': policy['status_body_bytes'] = True
    if mutation == 'zero_cap': policy['metrics_body_bytes'] = 0
    if mutation == 'status_ceiling': policy['status_body_bytes'] = 1024 * 1024 + 1
    if mutation == 'metrics_ceiling': policy['metrics_body_bytes'] = 16 * 1024 * 1024 + 1
    if mutation == 'oversubscribed_experiment': policy['metrics_body_bytes'] = 16 * 1024 * 1024
    if mutation == 'peer_geometry':
        for run in experiment['runs']: run['geometry']['peers'] = 5
    if mutation == 'missing_run': experiment['runs'].pop()
    if mutation == 'wrong_pair': budget['pair_index'] = 6
    if mutation == 'computed_summary': budget['resource_bytes'] = 0
    if mutation == 'extra_policy': value['capture_policy'] = dict(policy)
    path = write_config(tmp_path, value)
    calls = []
    def forbidden(*_):
        calls.append('owner')
        raise AssertionError('unadmitted owner reached')
    monkeypatch.setattr(worker, 'CaptureDirectory', forbidden)
    monkeypatch.setattr(worker, 'DarwinProcessReader', forbidden)
    monkeypatch.setattr(worker, 'ExecutableImage', forbidden)
    capture_path = tmp_path / 'must_not_be_created'
    output = io.BytesIO()
    code = worker.run_worker(path, capture_path,
        io.BytesIO(encoded(request('admit', 0)) + encoded(request('finish', 0))), output)
    assert code == 0
    assert calls == []
    assert not capture_path.exists()
    rows = [json.loads(line) for line in output.getvalue().splitlines()]
    assert [row['outcome'] for row in rows] == ['failed', 'complete']
    assert all(row['admission' if row['kind'] == 'admit' else 'manifest'] is None for row in rows)
    assert b'runtime-only-secret' not in output.getvalue()


@pytest.mark.parametrize('role', ['status', 'metrics'])
@pytest.mark.parametrize('ordinal', [0, 3])
@pytest.mark.parametrize('extra', [0, 1])
def test_publisher_checks_every_body_cap_before_first_create(tmp_path, role, ordinal, extra):
    original = observation()
    policy = CapturePolicy(len(original.peers[0].status.raw_body), len(original.peers[0].metrics.raw_body))
    selected = allocation(policy=policy)
    peer = original.peers[ordinal]
    source = getattr(peer, role)
    raw = source.raw_body + b' ' * extra
    modified = replace(source, raw_body=raw, body_bytes=len(raw), body_sha256=hashlib.sha256(raw).hexdigest())
    peers = list(original.peers)
    peers[ordinal] = replace(peer, **{role: modified})
    observed = replace(original, peers=tuple(peers), capture_policy=policy)
    path = capture_dir(tmp_path)
    with worker.CaptureDirectory(path, selected) as directory:
        if extra:
            with pytest.raises(probe.ProbeError, match='capture_body_policy_exceeded'):
                directory.publish('preflight', 0, observed, probe._Deadline(1))
            assert list(path.iterdir()) == []
            with pytest.raises(probe.ProbeError, match='capture_owner_failed'):
                directory.publish('preflight', 0, replace(original, capture_policy=policy), probe._Deadline(1))
        else:
            reference = directory.publish('preflight', 0, observed, probe._Deadline(1))
            manifest = json.loads(validate_reference(path, vars(reference)))
            assert manifest['capture_policy'] == {'status_body_bytes': policy.status_body_bytes,
                                                 'metrics_body_bytes': policy.metrics_body_bytes}
            assert validate_reference(path, manifest['peers'][ordinal][role]['body']) == raw
            assert len(list(path.iterdir())) == selected.members_per_capture


@pytest.mark.parametrize('mutation', ['policy', 'missing_policy', 'raw_hash', 'raw_size', 'route', 'wire_cap', 'wire_understates', 'peer_count', 'duplicate_peer'])
def test_publisher_rejects_observation_substitution_without_partial_files(tmp_path, mutation):
    observed = observation()
    peer = observed.peers[-1]
    if mutation == 'policy': observed = replace(observed, capture_policy=CapturePolicy())
    if mutation == 'missing_policy': observed = replace(observed, capture_policy=None)
    if mutation == 'raw_hash': peer = replace(peer, metrics=replace(peer.metrics, body_sha256='0' * 64))
    if mutation == 'raw_size': peer = replace(peer, metrics=replace(peer.metrics, body_bytes=1))
    if mutation == 'route': peer = replace(peer, metrics=replace(peer.metrics, route='/status'))
    if mutation == 'wire_cap': observed = replace(observed, wire_bytes=64 * 1024 * 1024 + 1)
    if mutation == 'wire_understates': observed = replace(observed, wire_bytes=1)
    if mutation == 'duplicate_peer': peer = replace(peer, peer_id=observed.peers[0].peer_id)
    if mutation in ('raw_hash', 'raw_size', 'route', 'duplicate_peer'):
        observed = replace(observed, peers=(*observed.peers[:-1], peer))
    if mutation == 'peer_count': observed = replace(observed, peers=observed.peers[:-1])
    path = capture_dir(tmp_path)
    with worker.CaptureDirectory(path, allocation()) as directory:
        with pytest.raises(probe.ProbeError):
            directory.publish('preflight', 0, observed, probe._Deadline(1))
        assert list(path.iterdir()) == []


def test_complete_real_capture_sequence_consumes_exact_member_and_byte_reservations(tmp_path):
    selected = allocation()
    observed = observation()
    path = capture_dir(tmp_path)
    with worker.CaptureDirectory(path, selected) as directory:
        for sequence in range(selected.capture_count):
            directory.publish('preflight' if sequence == 0 else 'sample', sequence, observed, probe._Deadline(1))
        actual = {file.name: (file.stat().st_size, hashlib.sha256(file.read_bytes()).hexdigest())
                  for file in path.iterdir()}
        assert directory._captures == selected.capture_count == 23
        assert directory._members == selected.member_count == len(actual) == 207
        assert directory._bytes == sum(size for size, _ in actual.values())
        assert directory._bytes <= selected.resource_bytes
        with pytest.raises(probe.ProbeError, match='capture_sequence_invalid'):
            directory.publish('sample', selected.capture_count, observed, probe._Deadline(1))
        assert {file.name: (file.stat().st_size, hashlib.sha256(file.read_bytes()).hexdigest())
                for file in path.iterdir()} == actual


@pytest.mark.parametrize('counter', ['_members', '_bytes'])
def test_tampered_cumulative_counter_cannot_exceed_re_admitted_reservation(tmp_path, counter):
    selected = allocation()
    path = capture_dir(tmp_path)
    with worker.CaptureDirectory(path, selected) as directory:
        setattr(directory, counter, selected.member_count if counter == '_members' else selected.resource_bytes)
        with pytest.raises(probe.ProbeError, match='capture_run_reservation_exceeded'):
            directory.publish('preflight', 0, observation(), probe._Deadline(1))
        assert list(path.iterdir()) == []


def test_partial_physical_write_consumes_reservation_and_cannot_retry_owner(tmp_path, monkeypatch):
    selected = allocation()
    path = capture_dir(tmp_path)
    real_write = worker.os.write
    calls = []
    def partial(fd, raw):
        if not calls:
            calls.append(True)
            return real_write(fd, raw[:1])
        raise OSError('synthetic partial write')
    with worker.CaptureDirectory(path, selected) as directory:
        monkeypatch.setattr(worker.os, 'write', partial)
        with pytest.raises(OSError):
            directory.publish('preflight', 0, observation(), probe._Deadline(1))
        assert directory._captures == 1
        assert directory._members == selected.members_per_capture
        assert directory._bytes == sum(len(raw) for _, raw, _ in directory._pending)
        assert len(list(path.iterdir())) == 1
        assert next(path.iterdir()).read_bytes() == observation().peers[0].status.raw_body[:1]
        with pytest.raises(probe.ProbeError, match='capture_owner_failed'):
            directory.publish('sample', 1, observation(), probe._Deadline(1))
        assert len(list(path.iterdir())) == 1


def test_raw_sink_requires_the_actual_complete_capture_reservation(tmp_path):
    path = capture_dir(tmp_path)
    with worker.CaptureDirectory(path, allocation()) as directory:
        assert not hasattr(directory, 'write')
        with pytest.raises(probe.ProbeError, match='capture_reservation_required'):
            directory._write_reserved('preflight-0000000000.json', b'{}', 100, probe._Deadline(1))
        assert list(path.iterdir()) == []


@pytest.mark.parametrize('mutation', ['equal_allocation', 'changed_summary', 'changed_peer_lifetime'])
def test_later_capture_cannot_change_admitted_allocation_or_peer_scope(tmp_path, mutation):
    selected = allocation()
    path = capture_dir(tmp_path)
    with worker.CaptureDirectory(path, selected) as directory:
        directory.publish('preflight', 0, observation(), probe._Deadline(1))
        before = {file.name: file.read_bytes() for file in path.iterdir()}
        observed = observation()
        if mutation == 'equal_allocation': directory.allocation = allocation()
        if mutation == 'changed_summary': object.__setattr__(selected.experiment, 'resource_bytes_per_run', 1)
        if mutation == 'changed_peer_lifetime':
            peer = observed.peers[-1]
            sample = replace(peer.process_before, identity=replace(peer.process_before.identity, start_abstime=2))
            peer = replace(peer, process_before=sample, process_after=sample)
            observed = replace(observed, peers=(*observed.peers[:-1], peer))
        with pytest.raises(probe.ProbeError):
            directory.publish('sample', 1, observed, probe._Deadline(1))
        assert {file.name: file.read_bytes() for file in path.iterdir()} == before


def test_allocation_change_during_body_publication_cannot_publish_manifest(tmp_path, monkeypatch):
    path = capture_dir(tmp_path)
    original = worker.os.fsync
    changed = []
    def change_after_first_sync(fd):
        original(fd)
        if not changed:
            changed.append(True)
            object.__setattr__(selected.experiment.policy, 'status_body_bytes', POLICY.status_body_bytes + 1)
    # Use an allocation with its own immutable policy object, so this deliberate
    # mutation cannot change the module-wide fixture policy for later tests.
    selected = allocation(policy=CapturePolicy(POLICY.status_body_bytes, POLICY.metrics_body_bytes))
    with worker.CaptureDirectory(path, selected) as directory:
        monkeypatch.setattr(worker.os, 'fsync', change_after_first_sync)
        with pytest.raises(probe.ProbeError, match='resource_budget_(?:invalid|changed)'):
            directory.publish('preflight', 0, observation(), probe._Deadline(1))
        assert changed == [True]
        assert not (path / 'preflight-0000000000.json').exists()
        assert len(list(path.iterdir())) == 8
        with pytest.raises(probe.ProbeError, match='capture_owner_failed'):
            directory.publish('sample', 1, observation(), probe._Deadline(1))


class AdmissionDialogue:
    """Parent-side request steps observe the actual prior flushed worker reply."""
    def __init__(self, output, steps):
        self.output, self.steps = output, iter(steps)

    def readline(self, _cap):
        try:
            step = next(self.steps)
        except StopIteration:
            return b''
        return encoded(step(self.output.getvalue()) if callable(step) else step)


def test_admission_ack_precedes_parent_directory_creation_and_retains_exact_config(tmp_path):
    config_path = write_config(tmp_path, config())
    capture_path = tmp_path / 'after-admission'
    output = io.BytesIO()
    calls = []
    def build(path, _owners):
        value = worker.read_config(path)
        calls.append(value)
        assert not capture_path.exists()
        return configured()
    def parent_create(prior):
        reply = json.loads(prior)
        assert reply == admission_reply(allocation())
        assert not capture_path.exists()
        # The admitted values remain owned by this child; preflight must not
        # reopen a changed runtime configuration or construct another collector.
        config_path.write_bytes(b'{}')
        capture_path.mkdir(mode=0o700)
        return request('preflight', 0)
    stream = AdmissionDialogue(output, [request('admit', 0), parent_create, request('finish', 1)])
    assert worker.run_worker(config_path, capture_path, stream, output, probe_builder=build) == 0
    rows = [json.loads(line) for line in output.getvalue().splitlines()]
    assert [row['outcome'] for row in rows] == ['complete', 'complete', 'complete']
    assert len(calls) == 1
    assert calls[0] == config()
    manifest = json.loads(validate_reference(capture_path, rows[1]['manifest']))
    assert manifest['capture_policy'] == calls[0]['resource_budget']['experiment']['capture_policy']
    assert len(list(capture_path.iterdir())) == 9
    assert b'runtime-only-secret' not in output.getvalue()


@pytest.mark.parametrize('kind,sequence', [('preflight', 0), ('sample', 1), ('finish', 0), ('admit', 1)])
def test_missing_or_wrong_admission_never_reaches_config_or_capture(tmp_path, kind, sequence):
    calls = []
    path = tmp_path / 'absent'
    output = io.BytesIO()
    code = worker.run_worker(tmp_path / 'unused', path, io.BytesIO(encoded(request(kind, sequence))),
                             output, probe_builder=lambda *_: calls.append('unexpected'))
    assert code == 1
    assert calls == []
    assert not path.exists()
    assert json.loads(output.getvalue())['outcome'] == 'failed'


@pytest.mark.parametrize('kind,sequence', [('admit', 0), ('preflight', 0), ('sample', 1), ('finish', 1), ('finish', 0)])
def test_failed_admission_is_poisoned_except_exact_finish_zero(tmp_path, kind, sequence):
    calls = []
    path = tmp_path / 'absent'
    output = io.BytesIO()
    def fail(*_):
        calls.append('admitted-once')
        raise OSError('Bearer NeverExpose /private/config')
    stream = io.BytesIO(encoded(request('admit', 0)) + encoded(request(kind, sequence)))
    code = worker.run_worker(tmp_path / 'unused', path, stream, output, probe_builder=fail)
    rows = [json.loads(line) for line in output.getvalue().splitlines()]
    finish = kind == 'finish' and sequence == 0
    assert code == (0 if finish else 1)
    assert calls == ['admitted-once']
    assert [row['outcome'] for row in rows] == ['failed', 'complete' if finish else 'failed']
    assert all(row['admission' if row['kind'] == 'admit' else 'manifest'] is None for row in rows)
    assert not path.exists()
    assert b'NeverExpose' not in output.getvalue()


@pytest.mark.parametrize('delay_ns,success', [(999_999_999, True), (1_000_000_000, False), (1_000_000_001, False)])
def test_admission_parent_setup_and_publication_share_one_absolute_deadline(tmp_path, monkeypatch, delay_ns, success):
    clock = [10_000_000_000]
    monkeypatch.setattr(probe.time, 'monotonic_ns', lambda: clock[0])
    path = tmp_path / 'captures'
    output = io.BytesIO()
    calls = []
    def build(*_):
        calls.append('config')
        clock[0] += 400_000_000
        return configured(collect=lambda _: (calls.append('http'), observation())[1])
    def parent(prior):
        assert json.loads(prior)['outcome'] == 'complete'
        assert not path.exists()
        clock[0] += delay_ns - 400_000_000
        path.mkdir(mode=0o700)
        return request('preflight', 0)
    stream = AdmissionDialogue(output, [request('admit', 0), parent, request('finish', 1)])
    assert worker.run_worker(tmp_path / 'unused', path, stream, output, probe_builder=build) == 0
    rows = [json.loads(line) for line in output.getvalue().splitlines()]
    assert rows[1]['outcome'] == ('complete' if success else 'failed')
    assert calls == (['config', 'http'] if success else ['config'])
    assert len(list(path.iterdir())) == (9 if success else 0)


@pytest.mark.parametrize('timeout', [500, 1001, 60000])
def test_preflight_cannot_replace_admission_timeout_or_retry_admission(tmp_path, timeout):
    path = capture_dir(tmp_path)
    output = io.BytesIO()
    calls = []
    def build(*_):
        calls.append('once')
        return configured()
    stream = io.BytesIO(encoded(request('admit', 0)) + encoded(request('preflight', 0, timeout_ms=timeout))
                        + encoded(request('finish', 1)))
    assert worker.run_worker(tmp_path / 'unused', path, stream, output, probe_builder=build) == 0
    rows = [json.loads(line) for line in output.getvalue().splitlines()]
    assert [row['outcome'] for row in rows] == ['complete', 'failed', 'complete']
    assert calls == ['once']
    assert list(path.iterdir()) == []


def test_valid_admission_cannot_be_repeated_before_preflight(tmp_path):
    path = tmp_path / 'absent'
    calls = []
    output = io.BytesIO()
    stream = io.BytesIO(encoded(request('admit', 0)) * 2)
    assert worker.run_worker(tmp_path / 'unused', path, stream, output,
                             probe_builder=lambda *_: (calls.append('once'), configured())[1]) == 1
    assert calls == ['once']
    assert [json.loads(line)['outcome'] for line in output.getvalue().splitlines()] == ['complete', 'failed']
    assert not path.exists()


def test_allocation_mutation_after_ack_cannot_publish_or_re_admit(tmp_path):
    path = capture_dir(tmp_path)
    selected = configured()
    output = io.BytesIO()
    def after_ack(prior):
        assert json.loads(prior)['outcome'] == 'complete'
        object.__setattr__(selected.allocation.experiment, 'resource_bytes_per_run', 1)
        return request('preflight', 0)
    stream = AdmissionDialogue(output, [request('admit', 0), after_ack, request('finish', 1)])
    assert worker.run_worker(tmp_path / 'unused', path, stream, output,
                             probe_builder=lambda *_: selected) == 0
    rows = [json.loads(line) for line in output.getvalue().splitlines()]
    assert rows[1]['outcome'] == 'failed'
    assert rows[1]['manifest'] is None
    assert list(path.iterdir()) == []


@pytest.mark.parametrize('tail_ns,success', [(399_999_999, True), (400_000_000, False)])
def test_actual_preflight_work_spends_remaining_admission_deadline(tmp_path, monkeypatch, tail_ns, success):
    clock = [10_000_000_000]
    monkeypatch.setattr(probe.time, 'monotonic_ns', lambda: clock[0])
    path = tmp_path / 'after-ack'
    output = io.BytesIO()
    def collect(_):
        clock[0] += tail_ns
        return observation()
    def build(*_):
        clock[0] += 400_000_000
        return configured(collect=collect)
    def parent(prior):
        assert json.loads(prior)['outcome'] == 'complete'
        assert not path.exists()
        clock[0] += 200_000_000
        path.mkdir(mode=0o700)
        return request('preflight', 0)
    stream = AdmissionDialogue(output, [request('admit', 0), parent, request('finish', 1)])
    assert worker.run_worker(tmp_path / 'unused', path, stream, output, probe_builder=build) == 0
    rows = [json.loads(line) for line in output.getvalue().splitlines()]
    assert rows[1]['outcome'] == ('complete' if success else 'failed')
    assert len(list(path.iterdir())) == (9 if success else 0)


@pytest.mark.parametrize('pair,variant', [(1, 'one_lane'), (5, 'four_lane')])
def test_admission_receipt_binds_complete_public_budget_and_roles_without_output(tmp_path, pair, variant):
    admitted = select_run_budget(allocation().experiment, pair, variant)
    config_owner = worker.ConfiguredProbe(SimpleNamespace(collect=lambda _: observation()), admitted)
    output = io.BytesIO()
    path = tmp_path / 'must-not-be-created'
    with ExitStack() as owners:
        ack = worker._admit(tmp_path / 'unused', owners, 1000, lambda *_: config_owner)
        raw = worker._admission_response(ack)
    value = json.loads(raw)
    assert value == admission_reply(admitted)
    assert set(value) == {'schema', 'kind', 'sequence', 'outcome', 'admission'}
    assert value['admission']['budget_sha256'] == hashlib.sha256(canonical_run_budget_bytes(admitted)).hexdigest()
    assert value['admission']['journal'] != value['admission']['trace']
    assert len(raw) <= worker.MAX_FRAME_BYTES and raw.endswith(b'\n')
    assert b'headers' not in raw and b'endpoint' not in raw and b'executable' not in raw
    assert not path.exists()


@pytest.mark.parametrize('replacement', ['same_callable', 'different_callable'])
def test_original_collector_replacement_after_admission_ack_fails_without_capture(tmp_path, replacement):
    original = configured()
    output = io.BytesIO()
    capture_path = tmp_path / 'captures-after-ack'
    called = []
    def after_ack(prior):
        assert json.loads(prior) == admission_reply(original.allocation)
        new_collect = original.collector.collect if replacement == 'same_callable' else lambda _: called.append('replacement')
        object.__setattr__(original, 'collector', SimpleNamespace(collect=new_collect))
        return request('preflight', 0)
    stream = AdmissionDialogue(output, [request('admit', 0), after_ack, request('finish', 1)])
    assert worker.run_worker(tmp_path / 'unused', capture_path, stream, output,
                             probe_builder=lambda *_: original) == 0
    rows = [json.loads(line) for line in output.getvalue().splitlines()]
    assert [row['outcome'] for row in rows] == ['complete', 'failed', 'complete']
    assert rows[1]['manifest'] is None and called == [] and not capture_path.exists()


def test_receipt_encoding_failure_poisoned_admission_still_permits_only_finish(tmp_path, monkeypatch):
    output = io.BytesIO()
    path = tmp_path / 'absent'
    monkeypatch.setattr(worker, 'run_budget_sha256', lambda _: (_ for _ in ()).throw(RuntimeError('secret')))
    stream = io.BytesIO(encoded(request('admit', 0)) + encoded(request('finish', 0)))
    assert worker.run_worker(tmp_path / 'unused', path, stream, output, probe_builder=lambda *_: configured()) == 0
    rows = [json.loads(line) for line in output.getvalue().splitlines()]
    assert rows[0] == {'schema': worker.RESPONSE_SCHEMA, 'kind': 'admit', 'sequence': 0, 'outcome': 'failed', 'admission': None}
    assert rows[1]['outcome'] == 'complete' and rows[1]['manifest'] is None
    assert b'secret' not in output.getvalue() and not path.exists()
