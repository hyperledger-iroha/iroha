"""Parent-record custody and canonical cache-sealer dispatch without children."""
from __future__ import annotations

import ast
import hashlib
import inspect
import json
import os
from pathlib import Path
import sys
import types

import pytest

ROOT = Path(__file__).resolve().parents[2]
MAIN = 'copy_sumeragi_v2_release_cargo_cache.py'
CLI = 'copy_sumeragi_v2_release_cargo_cache_cli.py'


def definitions(name):
    path = ROOT / 'scripts' / name
    parsed = ast.parse(path.read_bytes())
    body = [node for node in parsed.body if isinstance(node, (
        ast.Import, ast.ImportFrom, ast.FunctionDef, ast.ClassDef, ast.Assign, ast.AnnAssign))]
    module = types.ModuleType('scaling_cache_' + name[:-3])
    module.__file__ = str(path)
    sys.modules[module.__name__] = module
    exec(compile(ast.Module(body=body, type_ignores=[]), str(path), 'exec'), module.__dict__)
    return module


@pytest.mark.parametrize('change', ('none', 'exit', 'stdout', 'stderr', 'mode'))
def test_framework_probe_uses_canonical_isolation_and_exact_result(tmp_path, change):
    m = definitions(CLI)
    runtime = tmp_path / 'runtime'
    executable = runtime / 'bin/python3'
    executable.parent.mkdir(parents=True)
    executable.write_bytes(b'explicit process seam: no interpreter is executed')
    executable.chmod(0o500 if change != 'mode' else 0o700)
    preflights, calls = [], []
    m._preflight_macho = lambda *args, **kwargs: preflights.append((args, kwargs))
    stdout = os.fsencode(str(executable)) + b'\n'

    def run(*args, **kwargs):
        calls.append((args, kwargs))
        return types.SimpleNamespace(
            returncode=1 if change == 'exit' else 0,
            stdout=b'wrong\n' if change == 'stdout' else stdout,
            stderr=b'unexpected' if change == 'stderr' else b'')

    m._macho_run = run
    kwargs = dict(runtime_root=runtime, stdlib_name='python3.12',
                  probe_code='explicit probe fixture', error_type=ValueError)
    if change == 'none':
        assert m.probe_framework_python_runtime(**kwargs) == stdout
    else:
        with pytest.raises(ValueError, match='metadata is unsafe' if change == 'mode'
                           else 'isolated probe did not report its executable'):
            m.probe_framework_python_runtime(**kwargs)
    assert preflights == [((executable, ValueError), {'source': False})]
    if change == 'mode':
        assert calls == []
        return
    expected_zip = runtime / 'lib' / (
        f'python{sys.version_info.major}{sys.version_info.minor}.zip')
    assert calls == [(([
        str(executable), '-I', '-B', '-S', '-c', 'explicit probe fixture',
        str(executable), str(runtime), str(expected_zip),
        str(runtime / 'lib/python3.12'), str(runtime / 'lib/python3.12/lib-dynload')],
        runtime, ValueError), {
            'environment': {'LANG': 'C', 'LC_ALL': 'C', 'PATH': str(runtime / 'bin')},
            'maximum_output_bytes': 4096, 'label': 'archived interpreter probe'})]


@pytest.fixture
def record(tmp_path):
    root = tmp_path / 'bootstrap'
    root.mkdir(mode=0o700)
    path = root / 'scaling-execution.json'
    raw = b'{"explicit_fixture":"record semantics belong to receipt owner"}\n'
    path.write_bytes(raw)
    path.chmod(0o400)
    return root, path, raw, hashlib.sha256(raw).hexdigest()


def test_binds_actual_protected_parent_bytes(record):
    root, path, raw, digest = record
    assert definitions(MAIN)._scaling_execution_binding(path, digest, root) == {
        'archive_id': 'release-scaling.parent-execution.v1', 'sha256': digest,
        'size_bytes': len(raw), 'mode': '0400'}


@pytest.mark.parametrize('change', ('path', 'hash', 'empty', 'writable', 'symlink', 'hardlink', 'oversized'))
def test_rejects_changed_or_noncanonical_parent_record(record, change):
    root, path, raw, digest = record
    m = definitions(MAIN)
    if change == 'path':
        replacement = root / 'another.json'
        path.rename(replacement)
        path = replacement
    elif change == 'hash':
        digest = '0'*64
    elif change == 'empty':
        path.chmod(0o600)
        path.write_bytes(b'')
        path.chmod(0o400)
        digest = hashlib.sha256(b'').hexdigest()
    elif change == 'writable':
        path.chmod(0o600)
    elif change == 'symlink':
        target = root / 'target'
        path.rename(target)
        path.symlink_to(target)
    elif change == 'hardlink':
        os.link(path, root / 'alias')
    else:
        path.chmod(0o600)
        with path.open('wb') as stream:
            stream.truncate(m.MAX_SCALING_EXECUTION_RECORD_BYTES + 1)
        path.chmod(0o400)
    with pytest.raises((m.CacheCopyError, OSError)):
        m._scaling_execution_binding(path, digest, root)


def arguments(root):
    return ['--seal-release-result', '--invocation-root', str(root/'invocation'),
        '--bootstrap-evidence', str(root/'bootstrap'), '--source-manifest-sha256', '1'*64,
        '--candidate-root', str(root/'candidate'), '--scaling-execution-record',
        str(root/'bootstrap/scaling-execution.json'), '--expected-signer-fingerprint',
        'SHA256:'+'A'*43, '--expected-scaling-execution-sha256', '2'*64]


def dispatch(m, argv, calls):
    def forbidden(*args, **kwargs):
        raise AssertionError('unrelated operation was selected')
    callbacks = {name: forbidden for name in inspect.signature(m.run).parameters
                 if name not in ('argv', 'error_type')}
    callbacks['seal_release_result'] = lambda *args: calls.append(args)
    return m.run(error_type=ValueError, argv=argv, **callbacks)


def test_cli_dispatches_only_the_new_complete_parent_record_contract(tmp_path):
    m = definitions(CLI)
    calls = []
    assert dispatch(m, arguments(tmp_path), calls) == 0
    assert calls == [(tmp_path/'invocation', tmp_path/'bootstrap', '1'*64,
        tmp_path/'candidate', tmp_path/'bootstrap/scaling-execution.json',
        'SHA256:'+'A'*43, '2'*64)]


@pytest.mark.parametrize('option', ('--scaling-execution-record', '--expected-scaling-execution-sha256'))
def test_missing_parent_record_input_prevents_any_sealing(tmp_path, option):
    argv = arguments(tmp_path)
    index = argv.index(option)
    del argv[index:index+2]
    calls = []
    assert dispatch(definitions(CLI), argv, calls) == 1
    assert calls == []


@pytest.mark.parametrize('option', ('--scaling-evidence-manifest',
    '--expected-scaling-trial-harness-sha256', '--expected-scaling-configuration-sha256',
    '--expected-scaling-irohad-sha256', '--expected-scaling-iroha-cli-sha256'))
def test_retired_scaling_inputs_are_rejected(tmp_path, option):
    calls = []
    with pytest.raises(SystemExit) as raised:
        dispatch(definitions(CLI), arguments(tmp_path)+[option, 'retired'], calls)
    assert raised.value.code == 2 and calls == []


def test_sealer_retains_parent_file_through_all_existing_publication_fences():
    source = (ROOT/'scripts'/MAIN).read_text()
    function = next(node for node in ast.parse(source).body
                    if isinstance(node, ast.FunctionDef) and node.name == 'seal_release_result')
    text = ast.get_source_segment(source, function)
    assert 'held_files.append(scaling_held)' in text
    assert text.count('for held in held_files:') == 2
    assert 'for held in reversed(held_files):' in text
    assert text.index('held_files.append(scaling_held)') < text.index('_validation_ack(')


def test_shell_receipt_and_sealer_use_the_same_original_parent_record():
    shell = (ROOT/'scripts/run_sumeragi_v2_release_gates.sh').read_text()
    assert shell.count('--scaling-execution-record "$release_scaling_execution_record"') == 2
    assert shell.count('--expected-scaling-execution-sha256 "$release_scaling_execution_sha256"') == 2
    assert shell.index('exec {release_gate_fd}<&-') < shell.index('readonly release_scaling_execution_record=')
    assert 'parent scaling observation receipt join is not integrated' not in shell
    assert 'original protected parent runs the complete source-bound scaling preflight' in shell


def test_normalized_options_put_record_and_digest_together():
    m = definitions(MAIN)
    options = m.VALIDATOR_OPTION_ORDER
    offset = options.index('--g12-fault-soak-completion')
    assert options[offset+1:offset+4] == ('--scaling-execution-record',
        '--expected-scaling-execution-sha256', '--sdk-dependency-archive')
    assert '--scaling-execution-record' in m.VALIDATOR_PATH_OPTIONS
    assert '--expected-scaling-execution-sha256' not in m.VALIDATOR_PATH_OPTIONS


@pytest.mark.parametrize('tamper', (False, True))
def test_actual_ack_reader_joins_parent_record_and_exact_invocation(record, tmp_path, tamper):
    root, parent_record, parent_raw, parent_sha = record
    m = definitions(MAIN)
    namespace = m._validation_component(ROOT)
    function = namespace['_validation_ack']
    def write(path, value):
        raw = value if type(value) is bytes else m._canonical_payload(value)
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists(): path.chmod(0o600)
        path.write_bytes(raw); path.chmod(0o400)
        return {'path': path, 'data': raw, 'metadata': path.stat()}
    validator = write(root/'validate-receipt.py', b'# inert protected validator\n')
    completion = write(root/'BOOTSTRAP_COMPLETED.json', {'trusted_inputs': {
        name: {'sha256': str(index)*64} for index, name in enumerate(
            ('git', 'ssh_keygen', 'allowed_signers', 'revocation'), start=1)}})
    source = tmp_path/'invocation/source'
    ack_path = source.parent/'receipt-validation-ack.json'
    receipt_path = source.parent/'output/release/RELEASE_COMPLETED.json'
    path_record = lambda label: {'path': str(source.parent/label)}
    evidence = {name: path_record(name) for name in ('corridor_completion',
        'formal_completion', 'seed_matrix_completion', 'chaos_completion')}
    evidence.update(formal_replay_release={'signature': {'sha256': 'a'*64},
        'principal': 'release', 'source_receipt': path_record('formal-source'),
        'receipt': path_record('formal/receipt.json')},
        g4p_multilane={'completion': path_record('g4')},
        g12_cross_dataspace={'seed_completion': path_record('g12-seeds'),
                            'fault_soak_completion': path_record('g12-soak')},
        multilane_scaling={'parent_execution': m._scaling_execution_binding(parent_record, parent_sha, root)})
    receipt = write(receipt_path, {'authentication': {'bootstrap': {}}, 'evidence': evidence})
    ack = write(ack_path, {})
    captured = {}
    class Captured(Exception): pass
    def capture(_value, *, expected_values):
        captured.update(expected_values)
        raise Captured()
    namespace['_validate_validator_invocation'] = capture
    args = (source, root, 'b'*64, tmp_path/'candidate', parent_record, 'SHA256:'+'A'*43, parent_sha)
    with pytest.raises(Captured): function(ack, receipt, *args)
    assert captured['--scaling-execution-record'] == ('path', str(parent_record))
    assert captured['--expected-scaling-execution-sha256'] == ('text', parent_sha)
    assert set(captured) == set(m.VALIDATOR_OPTION_ORDER)
    invocation = {'profile': 'release', 'operation': 'verify-existing-and-ack',
        'python_flags': ['-I', '-B', '-S'], 'validator': 'protected:validate-receipt.py',
        'ordered_options': [{'name': name, 'value_kind': captured[name][0],
            'normalized_value_sha256': m._validator_invocation_value_sha256(*captured[name])}
            for name in m.VALIDATOR_OPTION_ORDER]}
    invocation['invocation_sha256'] = hashlib.sha256(json.dumps(invocation,
        ensure_ascii=False, separators=(',', ':'), sort_keys=True).encode()).hexdigest()
    sha = lambda raw: hashlib.sha256(raw).hexdigest()
    stdout = f'Sumeragi v2 aggregate release receipt verified: {receipt_path}\n'.encode()
    value = {'format': 'iroha-sumeragi-v2-receipt-validation-ack', 'schema_version': 3,
        'profile': 'release', 'sealed_source': {'archive_id': 'release-retained.source.v1',
            'manifest_sha256': 'b'*64}, 'receipt': {'archive_id': 'release-terminal.receipt.v1',
            'mode': '0400', 'sha256': sha(receipt['data']), 'size_bytes': len(receipt['data'])},
        'validator': {'archive_id': 'release-bootstrap.receipt-validator.v1',
            'sha256': sha(validator['data']), 'bootstrap_completion_sha256': sha(completion['data'])},
        'invocation': invocation, 'exit_status': 0,
        'stdout': {'sha256': sha(stdout), 'size_bytes': len(stdout)},
        'stderr': {'sha256': sha(b''), 'size_bytes': 0}}
    ack = write(ack_path, value)
    namespace['_validate_validator_invocation'] = m._validate_validator_invocation
    if tamper:
        evidence['multilane_scaling']['parent_execution']['sha256'] = 'f'*64
        receipt = write(receipt_path, {'authentication': {'bootstrap': {}}, 'evidence': evidence})
        with pytest.raises(m.CacheCopyError, match='retained parent scaling execution'):
            function(ack, receipt, *args)
    else:
        assert function(ack, receipt, *args) == (sha(ack['data']), len(ack['data']))
