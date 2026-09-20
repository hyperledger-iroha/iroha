"""Select real retained build files with explicit manifest/rustc process seams."""
from __future__ import annotations

import ast
from dataclasses import FrozenInstanceError
import os
from pathlib import Path
import sys
import types

import pytest

ROOT = Path(__file__).resolve().parents[2]


def bootstrap_definitions():
    """Load source definitions without executing protected component startup."""
    path = ROOT / 'scripts/bootstrap_sumeragi_v2_release.py'
    parsed = ast.parse(path.read_bytes())
    body = [node for node in parsed.body if isinstance(node, (
        ast.Import, ast.ImportFrom, ast.ClassDef, ast.FunctionDef, ast.Assign, ast.AnnAssign))]
    module = types.ModuleType('scaling_source_selection_bootstrap')
    module.__file__ = str(path)
    sys.modules[module.__name__] = module
    exec(compile(ast.Module(body=body, type_ignores=[]), str(path), 'exec'), module.__dict__)
    return module


@pytest.fixture
def selection(tmp_path, monkeypatch):
    m = bootstrap_definitions()
    base = tmp_path / 'base'
    base.mkdir(mode=0o700)
    root = base / 'invocation'
    root.mkdir(mode=0o700)
    source = root / 'source'
    source.mkdir(mode=0o500)
    evidence = tmp_path / 'evidence'
    evidence.mkdir(mode=0o700)
    fd = os.open(evidence, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    owner = object.__new__(m.ReleaseInvocationRoot)
    owner._base = base
    owner._base_fd = os.open(base, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    info = os.fstat(owner._base_fd)
    owner._base_pin = (info.st_dev, info.st_ino, info.st_mode & 0o7777, info.st_uid)
    owner._root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    owner._snapshot = m._private_directory_snapshot(root, 'fixture original root')
    owner._closed = False
    original = dict(schema_version=1, head_commit='1'*40, head_tree='2'*40,
                    index_tree='2'*40, cargo_lock_sha256='3'*64,
                    workspace_source_manifest_sha256='4'*64)
    sealed = {**original, 'workspace_source_manifest_sha256': '5'*64}

    def write(path, raw, executable=False):
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists():
            path.chmod(0o600)
        path.write_bytes(raw)
        path.chmod(0o500 if executable else 0o400)
        return m._read_file(path, 'fixture input', maximum_bytes=1024*1024, executable=executable)

    candidate = write(evidence/'candidate.json', m._canonical_json(original))
    write(root/'sealed-identity.json', m._canonical_json(sealed))
    python = write(evidence/'python', b'inert Python fixture\n', True)
    helper = write(evidence/'manifest.py', b'# inert manifest fixture\n')
    rustc = write(evidence/'rustc', b'inert rustc fixture\n', True)
    binary = write(root/'output/sumeragi-v2-release'/('5'*64)/'programs/.sumeragi-v2-prebuilt-binaries.tsv',
                   b'fixture retained native manifest\n')
    observed = []
    calls = []

    def identity(*args):
        observed.append(args)
        return m._canonical_json(sealed), dict(sealed)

    def command(executable, argv, **kwargs):
        calls.append((executable, argv, kwargs))
        if executable == python.path:
            write(Path(argv[-1]), b'Cargo.lock\0scripts/example.py\0')
            return m.CommandResult(0, (sealed['workspace_source_manifest_sha256']+'\n').encode(), b'')
        assert executable == rustc.path
        return m.CommandResult(0, b'rustc 1.0\nhost: aarch64-apple-darwin\n', b'')

    monkeypatch.setattr(m, '_compute_identity', identity)
    monkeypatch.setattr(m, '_run_bounded', command)
    values = dict(invocation=owner, candidate_identity=candidate, python=python,
                  manifest_helper=helper, rustc=rustc, evidence=evidence, evidence_fd=fd,
                  framework_binding=b'{"fixture":"framework seam"}\n',
                  environment={'LANG': 'C', 'PATH': str(evidence)}, timeout_seconds=30)
    yield types.SimpleNamespace(m=m, values=values, sealed=sealed, original=original,
        source=source, binary=binary, calls=calls, observed=observed, write=write,
        command=command, identity=identity)
    owner.close()
    os.close(fd)
    source.chmod(0o700)


def test_selects_only_original_roots_and_accepts_permission_seal_digest_change(selection):
    s = selection
    result = s.m._prepare_scaling_source_selection(**s.values)
    assert result.source.path == s.source
    assert result.identity.data == s.m._canonical_json(s.sealed)
    assert result.binary_manifest == s.binary
    assert result.source_paths.path == s.values['evidence']/'scaling-source-paths.txt'
    assert result.rustc_version.data == b'rustc 1.0\nhost: aarch64-apple-darwin\n'
    assert result.python_runtime_binding.data == s.values['framework_binding']
    assert len(s.observed) == 2 and len(s.calls) == 2
    assert s.calls[0][1][:3] == ('-I', '-B', '-S')
    assert s.calls[0][1][3] == str(s.values['manifest_helper'].path)
    assert s.calls[0][2]['cwd'] == s.source
    assert s.calls[1][0] == s.values['rustc'].path
    assert s.calls[1][1] == ('--version', '--verbose')
    assert all(call[2]['timeout_seconds'] == 30 for call in s.calls)
    assert all('pass_fds' not in call[2] for call in s.calls)
    with pytest.raises(FrozenInstanceError):
        result.source = None


@pytest.mark.parametrize('field', ('head_commit', 'head_tree', 'index_tree', 'cargo_lock_sha256'))
def test_rejects_identity_drift_before_path_list_or_tool_execution(selection, field):
    s = selection
    s.sealed[field] = '9'*len(s.sealed[field])
    s.write(s.values['invocation'].path/'sealed-identity.json', s.m._canonical_json(s.sealed))
    with pytest.raises(s.m.BootstrapError):
        s.m._prepare_scaling_source_selection(**s.values)
    assert not s.calls


def test_rejects_shell_identity_not_reproduced_by_protected_helper(selection):
    s = selection
    s.write(s.values['invocation'].path/'sealed-identity.json', s.m._canonical_json(s.original))
    with pytest.raises(s.m.BootstrapError):
        s.m._prepare_scaling_source_selection(**s.values)
    assert not s.calls


@pytest.mark.parametrize('role', ('python', 'manifest_helper', 'rustc'))
def test_rejects_changed_selected_tool_before_execution(selection, role):
    s = selection
    s.write(s.values[role].path, b'changed original tool\n', role != 'manifest_helper')
    with pytest.raises(s.m.BootstrapError):
        s.m._prepare_scaling_source_selection(**s.values)
    assert not s.calls and not s.observed


@pytest.mark.parametrize('change', ('returncode', 'stderr', 'manifest_digest'))
def test_rejects_failed_or_wrong_path_list_publication(selection, monkeypatch, change):
    s = selection
    def altered(executable, argv, **kwargs):
        result = s.command(executable, argv, **kwargs)
        return s.m.CommandResult(1 if change == 'returncode' else 0,
            b'6'*64+b'\n' if change == 'manifest_digest' else result.stdout,
            b'error\n' if change == 'stderr' else b'')
    monkeypatch.setattr(s.m, '_run_bounded', altered)
    with pytest.raises(s.m.BootstrapError):
        s.m._prepare_scaling_source_selection(**s.values)
    assert len(s.calls) == 1


def test_rejects_source_changed_during_tool_observation(selection, monkeypatch):
    s = selection
    def changed(*args):
        raw, value = s.identity(*args)
        if len(s.observed) == 2:
            value['workspace_source_manifest_sha256'] = '9'*64
            raw = s.m._canonical_json(value)
        return raw, value
    monkeypatch.setattr(s.m, '_compute_identity', changed)
    with pytest.raises(s.m.BootstrapError):
        s.m._prepare_scaling_source_selection(**s.values)
    assert len(s.observed) == 2


def test_rejects_reused_source_path_list(selection):
    s = selection
    s.write(s.values['evidence']/'scaling-source-paths.txt', b'existing\n')
    with pytest.raises(s.m.BootstrapError):
        s.m._prepare_scaling_source_selection(**s.values)
    assert not s.calls


@pytest.mark.parametrize('output', (b'', b'not terminated', b'bad\0output\n', b'bad\r\n', b'\xff\n'))
def test_rejects_malformed_actual_rustc_output(selection, monkeypatch, output):
    s = selection
    def altered(executable, argv, **kwargs):
        if executable == s.values['rustc'].path:
            return s.m.CommandResult(0, output, b'')
        return s.command(executable, argv, **kwargs)
    monkeypatch.setattr(s.m, '_run_bounded', altered)
    with pytest.raises(s.m.BootstrapError):
        s.m._prepare_scaling_source_selection(**s.values)
    assert not (s.values['evidence']/'scaling-rustc-version.txt').exists()


def test_rejects_writable_source_before_manifest_execution(selection):
    s = selection
    s.source.chmod(0o700)
    with pytest.raises(s.m.BootstrapError):
        s.m._prepare_scaling_source_selection(**s.values)
    assert not s.calls and not s.observed


def test_rejects_symlinked_native_manifest(selection):
    s = selection
    target = s.values['evidence']/'foreign-manifest'
    s.write(target, s.binary.data)
    s.binary.path.unlink()
    s.binary.path.symlink_to(target)
    with pytest.raises(s.m.BootstrapError):
        s.m._prepare_scaling_source_selection(**s.values)
    assert len(s.calls) == 1
