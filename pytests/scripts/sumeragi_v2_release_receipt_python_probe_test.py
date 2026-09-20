"""Actual protected receipt argv and closed probe records through process seams.

The complete production functions and serialized-expectation block execute.
Only file capture and process execution are explicit seams; receipt runtime
modules are not imported during collection, and no real children are launched.
"""
from __future__ import annotations

import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace as NS

import pytest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'scripts/write_sumeragi_v2_release_receipt.py'


@pytest.fixture(autouse=True)
def no_process_or_signal(monkeypatch):
    def denied(*args, **kwargs):
        raise AssertionError('real process and signal operations are forbidden')
    monkeypatch.setattr(subprocess, 'Popen', denied)
    for name in ('system', 'fork', 'posix_spawn', 'posix_spawnp', 'kill', 'killpg'):
        if hasattr(os, name):
            monkeypatch.setattr(os, name, denied)


@pytest.fixture(scope='module')
def production():
    tree = ast.parse(SOURCE.read_text())
    wanted = {'ReceiptError', '_canonical_json', '_decode_canonical_json',
              '_require_exact_json_fields', '_run_bounded_python_validator',
              '_validate_and_replay_tool_probe_closure'}
    nodes = [node for node in tree.body if isinstance(node, (ast.ClassDef, ast.FunctionDef))
             and node.name in wanted]
    assert {node.name for node in nodes} == wanted
    constants = {'_MAX_REPLAY_OUTPUT_BYTES', '_MAX_HELPER_BYTES', '_MAX_TOOL_BYTES'}
    nodes[:0] = [node for node in tree.body if isinstance(node, ast.Assign)
                 and any(isinstance(target, ast.Name) and target.id in constants for target in node.targets)]
    future = ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)
    scope = dict(Path=Path, os=os, sys=sys, hashlib=hashlib, json=json)
    exec(compile(ast.fix_missing_locations(ast.Module(body=[future, *nodes], type_ignores=[])),
                 str(SOURCE), 'exec'), scope)
    function = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                    and node.name == '_validate_bootstrap_evidence')
    start = next(index for index, node in enumerate(function.body)
                 if isinstance(node, ast.Assign) and any(isinstance(target, ast.Name)
                    and target.id == 'probes' for target in node.targets))
    end = next(index for index, node in enumerate(function.body[start:], start)
               if isinstance(node, ast.Assign) and any(isinstance(target, ast.Name)
                    and target.id == 'tool_probe_closure' for target in node.targets))
    block = function.body[start:end]
    assert len(block) == 5
    compiled = compile(ast.fix_missing_locations(ast.Module(body=block, type_ignores=[])),
                       str(SOURCE), 'exec')
    return scope, compiled


@pytest.mark.parametrize('status', [0, 23])
def test_actual_python_validator_argv_and_custody(production, status):
    scope = dict(production[0]); checker = Path('/protected/checker.py')
    snapshot = NS(path=checker, data=b'print("captured source")\n')
    interpreter = NS(path=Path(sys.executable).resolve(strict=True))
    extra = NS(path=Path('/protected/extra')); calls = []
    def capture(path, label, **options):
        assert path == checker and options['require_single_link'] is True
        return snapshot
    def capture_interpreter(path, label, **options):
        assert path == interpreter.path and options['executable'] is True
        return interpreter
    def replay(executable, argv, **options):
        calls.append((executable, argv, options))
        return status, b'output', b'diagnostic'
    scope.update(_bounded_evidence_snapshot=capture, _bounded_path_contract=capture_interpreter,
                 _run_bounded_replay=replay)
    function = type(production[0]['_run_bounded_python_validator'])(
        production[0]['_run_bounded_python_validator'].__code__, scope,
        argdefs=production[0]['_run_bounded_python_validator'].__defaults__)
    function.__kwdefaults__ = production[0]['_run_bounded_python_validator'].__kwdefaults__
    result = function(checker, ['--input', '/protected/input'], cwd=Path('/protected'),
        environment={'LANG': 'C'}, name='protected validator', maximum_output_bytes=4096,
        watched_contracts=(extra,))
    assert result == (status, b'output', b'diagnostic') and len(calls) == 1
    executable, argv, options = calls[0]
    assert executable == interpreter.path and argv[:4] == ['-I', '-B', '-S', '-c']
    assert argv[5:] == [str(checker), '--input', '/protected/input']
    assert "exec(compile(source,path,'exec'),scope,scope)" in argv[4]
    assert options['stdin_data'] is snapshot.data
    assert options['watched_contracts'] == (snapshot, extra)
    assert options['executable_contract'] is interpreter and options['maximum_output_bytes'] == 4096
    assert options['environment'] == {'LANG': 'C'}


def tool_fixture(production, tmp_path, mutation=None):
    scope = dict(production[0]); canonical = scope['_canonical_json']; prefix = 'release-test-tool'
    tools = {f'tool{index:02d}': NS(path=tmp_path / f'tool{index:02d}', sha256=f'{index + 1:064x}', size=index + 1)
             for index in range(41)}
    manifest = dict(schema_version=1, tools={name: dict(archive_id=f'{prefix}.{name}.v1',
        path=str(tool.path), sha256=tool.sha256) for name, tool in tools.items()})
    result = dict(format='iroha-sumeragi-v2-release-tool-functional-probes',
        host_family='darwin' if sys.platform == 'darwin' else 'linux', probe_contract_sha256='a'*64,
        schema_version=1, tool_count=41, tools={name: dict(archive_id=f'{prefix}.{name}.v1',
            exit_status=0, invocation_sha256='b'*64, mode='0500', operation_id='probe.v1',
            postcondition_sha256='c'*64, sha256=tool.sha256, size_bytes=tool.size,
            stderr_sha256=hashlib.sha256(b'').hexdigest(), stderr_size_bytes=0,
            stdout_sha256=hashlib.sha256(b'ok').hexdigest(), stdout_size_bytes=2)
            for name, tool in tools.items()})
    expected = json.loads(canonical(result)); probe_root = tmp_path / 'probe-work'
    if mutation == 'tool_count': result['tool_count'] = 40
    elif mutation == 'manifest_layout': manifest['accepted'] = True
    elif mutation == 'result_layout': result['tools']['tool00']['compatibility'] = True
    elif mutation == 'tool_digest': result['tools']['tool00']['sha256'] = '0'*64
    snapshots = {
        tmp_path / 'manifest.json': NS(path=tmp_path/'manifest.json', data=canonical(manifest), sha256='d'*64),
        tmp_path / 'result.json': NS(path=tmp_path/'result.json', data=canonical(result), sha256='e'*64),
    }
    calls = []
    def capture(path, label, **options):
        assert options['expected_mode'] == 0o400 and options['maximum_bytes'] == 1024 * 1024
        return snapshots[path]
    def replay(executable, argv, **options):
        calls.append((executable, argv, options))
        stdout = snapshots[tmp_path/'result.json'].data
        if mutation == 'directory': probe_root.mkdir()
        if mutation == 'symlink': probe_root.symlink_to(tmp_path / 'absent')
        return (9 if mutation == 'status' else 0,
                stdout + (b'\n' if mutation == 'stdout' else b''),
                b'error' if mutation == 'stderr' else b'')
    scope.update(_bounded_evidence_snapshot=capture, _closed_replay_environment=lambda path: {'LANG': 'C'},
                 _run_bounded_replay=replay)
    original = scope['_validate_and_replay_tool_probe_closure']
    function = type(original)(original.__code__, scope, argdefs=original.__defaults__)
    function.__kwdefaults__ = original.__kwdefaults__
    python = NS(path=tmp_path/'python3'); helper = NS(path=tmp_path/'probe.py')
    args = dict(manifest_path=tmp_path/'manifest.json', result_path=tmp_path/'result.json',
        expected_value=expected, tools=tools, python=python, helper=helper,
        archive_id_prefix=prefix, probe_root=probe_root)
    return function, args, calls, snapshots, expected


def test_actual_tool_probe_argv_and_exact_reply(production, tmp_path):
    function, args, calls, snapshots, expected = tool_fixture(production, tmp_path)
    assert function(**args) == (snapshots[args['manifest_path']], snapshots[args['result_path']], expected)
    assert len(calls) == 1
    executable, argv, options = calls[0]
    assert executable == args['python'].path and argv[:3] == ['-I', '-B', '-S']
    assert argv[3:] == [str(args['helper'].path), '--tool-manifest', str(args['manifest_path']),
        '--expected-tool-manifest-sha256', 'd'*64, '--probe-root', str(args['probe_root'])]
    assert options['watched_contracts'] == (args['helper'], snapshots[args['manifest_path']], *args['tools'].values())
    assert options['executable_contract'] is args['python']


@pytest.mark.parametrize('mutation', ['status', 'stdout', 'stderr', 'directory', 'symlink',
                                    'tool_count', 'manifest_layout', 'result_layout', 'tool_digest'])
def test_tool_probe_retains_unmatched_result_rejection(production, tmp_path, mutation):
    function, args, calls, _, _ = tool_fixture(production, tmp_path, mutation)
    with pytest.raises(production[0]['ReceiptError']): function(**args)
    assert len(calls) == (0 if mutation in {'tool_count', 'manifest_layout', 'result_layout', 'tool_digest'} else 1)


def probe_record(*, framework=False):
    directory = Path('/protected/bootstrap')
    python = directory / ('python-runtime/bin/python3' if framework else 'python3')
    output = f'{python}\n'.encode(); code = "import sys;sys.stdout.write(sys.executable+'\\n')"
    probes = dict(bash=dict(argv=[str(directory/'bash'), '-c', ':'], exit_status=0),
        python=dict(argv=[str(python), '-I', '-B', '-S', '-c', code],
            expected_executable='python-runtime/bin/python3' if framework else 'python3',
            exit_status=0, stdout_sha256=hashlib.sha256(output).hexdigest(), stdout_size_bytes=len(output)),
        runner_tool_closure={})
    return dict(marker={'trusted_execution_probes': probes}, directory=directory,
                python_archive_path=python, framework_python=framework)


@pytest.mark.parametrize('framework', [False, True])
def test_serialized_probe_requires_exact_canonical_layout(production, framework):
    scope, block = production; state = dict(scope, **probe_record(framework=framework))
    exec(block, state)
    assert state['expected_probes']['python']['argv'][1:4] == ['-I', '-B', '-S']


@pytest.mark.parametrize('flags', [
    ['-I', '-S'], ['-I', '-S', '-B'], ['-I', '-B'], ['-B', '-S'],
    ['-I', '-B', '-S', '-B'], ['-I', '-B', '-S', '-X', 'dev'], [],
])
def test_alternate_python_probe_argv_is_rejected(production, flags):
    scope, block = production; state = dict(scope, **probe_record())
    argv = state['marker']['trusted_execution_probes']['python']['argv']
    state['marker']['trusted_execution_probes']['python']['argv'] = [argv[0], *flags, *argv[-2:]]
    with pytest.raises(scope['ReceiptError']): exec(block, state)


@pytest.mark.parametrize('mutation', ['boolean_status', 'wrong_output', 'wrong_executable', 'extra', 'missing'])
def test_serialized_probe_preserves_other_exact_assertions(production, mutation):
    scope, block = production; state = dict(scope, **probe_record())
    probe = state['marker']['trusted_execution_probes']['python']
    if mutation == 'boolean_status': probe['exit_status'] = False
    elif mutation == 'wrong_output': probe['stdout_sha256'] = '0'*64
    elif mutation == 'wrong_executable': probe['expected_executable'] = 'foreign-python'
    elif mutation == 'extra': probe['compatible'] = True
    else: del probe['stdout_size_bytes']
    with pytest.raises(scope['ReceiptError']): exec(block, state)
