"""Fixed public descriptor borrowing with original files and mocked native children."""
import fcntl
import hashlib
import os
from pathlib import Path

import pytest

from scaling_readiness_fixture import ready_setup, inputs
from scaling_native_load_test import load_setup
import scaling_native_load as load
from scaling_load_outputs import LoadOutputError


PUBLIC = ('genesis.json', 'genesis.signed.nrt', 'genesis-context.nrt',
          'genesis.expected_hash', 'genesis-anchors.json')
LOAD = ('collector_journal', 'transaction_trace')


def _completed(setup):
    owner = setup.create()
    receipt = setup.c.run.run_load(lambda _: owner.run())
    return owner, receipt


def _readonly(fd):
    assert fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE == os.O_RDONLY
    assert not os.get_inheritable(fd)


def _replace(path):
    raw = path.read_bytes()
    original = path.with_name(path.name + '.original')
    path.rename(original)
    path.write_bytes(raw)
    path.chmod(0o600)


@pytest.mark.parametrize('name', PUBLIC)
def test_genesis_borrow_uses_original_fd_and_duplicate_survives_source_close(ready_setup, monkeypatch, name):
    c = ready_setup
    expected = (c.directory / name).read_bytes()
    with monkeypatch.context() as patch:
        patch.setattr(os, 'open', lambda *a, **kw: pytest.fail('descriptor borrowing reopened a pathname'))
        fd = c.inputs.public_descriptor(name)
        assert fd == c.inputs._files[name][0]
        _readonly(fd)
        duplicate = os.dup(fd)
    try:
        c.inputs.close()
        _readonly(duplicate)
        assert os.pread(duplicate, len(expected) + 1, 0) == expected
        assert (c.directory / name).read_bytes() == expected
        with pytest.raises(OSError): os.fstat(fd)
        with pytest.raises(inputs.ReadinessError): c.inputs.public_descriptor(name)
    finally:
        os.close(duplicate)


@pytest.mark.parametrize('name', ['client.toml', 'peer0.toml', 'peer3-client.toml',
    'workload-account-00.toml', '../genesis.json', './genesis.json', '/genesis.json',
    'GENESIS.JSON', 'genesis.json.publishing', 'storage', '', None, b'genesis.json'])
def test_genesis_borrow_rejects_every_nonpublic_role_and_poison_is_permanent(ready_setup, name):
    c = ready_setup
    with pytest.raises(inputs.ReadinessError, match='^readiness_failed$'):
        c.inputs.public_descriptor(name)
    assert c.inputs._failed
    with pytest.raises(inputs.ReadinessError): c.inputs.public_descriptor('genesis.json')


@pytest.mark.parametrize('name', PUBLIC)
@pytest.mark.parametrize('change', ['replace', 'modify', 'mode', 'hardlink', 'remove'])
def test_genesis_borrow_rechecks_original_full_file_custody(ready_setup, name, change):
    c = ready_setup
    path = c.directory / name
    if change == 'replace': _replace(path)
    elif change == 'modify': path.write_bytes(path.read_bytes() + b' ')
    elif change == 'mode': path.chmod(0o644)
    elif change == 'hardlink': os.link(path, c.root / 'second-link')
    elif change == 'remove': path.unlink()
    with pytest.raises(inputs.ReadinessError): c.inputs.public_descriptor(name)
    assert c.inputs._failed


def test_genesis_borrow_rejects_replaced_original_parent(ready_setup):
    c = ready_setup
    c.directory.rename(c.directory.with_name('old-inputs'))
    c.directory.mkdir(mode=0o700)
    with pytest.raises(inputs.ReadinessError): c.inputs.public_descriptor('genesis.json')


@pytest.mark.parametrize('source', ['genesis', 'load'])
@pytest.mark.parametrize('kind', ['foreign_file', 'same_file_writable', 'foreign_directory'])
def test_reused_descriptor_is_rejected_and_close_preserves_replacement(
        ready_setup, load_setup, source, kind):
    c = ready_setup
    if source == 'genesis':
        owner, exception = c.inputs, inputs.ReadinessError
        role, path = 'genesis.json', c.directory / 'genesis.json'
        fd = owner.public_descriptor(role)
        if kind == 'foreign_directory': fd = owner._directories[c.directory][0]
    else:
        owner, _ = _completed(load_setup)
        exception, role = load.NativeLoadError, 'collector_journal'
        path = load_setup.paths.collector_journal
        fd = owner.public_descriptor(role)
        if kind == 'foreign_directory': fd = owner._files._directories[path.parent][0]
    replacement_path = path if kind == 'same_file_writable' else c.root / 'foreign-descriptor'
    if replacement_path != path:
        replacement_path.write_bytes(b'foreign resource retained by its actual owner')
    replacement = os.open(replacement_path, os.O_RDWR | os.O_CLOEXEC)
    os.dup2(replacement, fd, inheritable=False)
    os.close(replacement)
    try:
        with pytest.raises(exception): owner.public_descriptor(role)
        with pytest.raises(exception): owner.close()
        assert os.fstat(fd).st_ino == replacement_path.stat().st_ino
        assert os.pread(fd, 7, 0) == replacement_path.read_bytes()[:7]
        owner.close()  # No second close attempt can consume the foreign FD.
        assert os.fstat(fd).st_ino == replacement_path.stat().st_ino
    finally:
        os.close(fd)


@pytest.mark.parametrize('source', ['genesis', 'load'])
@pytest.mark.parametrize('flag', ['inherit', 'nonblock'])
def test_borrow_rejects_changed_original_descriptor_flags(ready_setup, load_setup, source, flag):
    if source == 'genesis':
        owner, role, exception = ready_setup.inputs, 'genesis.json', inputs.ReadinessError
    else:
        owner, _ = _completed(load_setup)
        role, exception = 'collector_journal', load.NativeLoadError
    fd = owner.public_descriptor(role)
    status = fcntl.fcntl(fd, fcntl.F_GETFL)
    if flag == 'inherit': os.set_inheritable(fd, True)
    else: fcntl.fcntl(fd, fcntl.F_SETFL, status ^ os.O_NONBLOCK)
    try:
        with pytest.raises(exception): owner.public_descriptor(role)
    finally:
        os.set_inheritable(fd, False)
        fcntl.fcntl(fd, fcntl.F_SETFL, status)


@pytest.mark.parametrize('role', LOAD)
def test_completed_load_borrow_retains_native_identity_and_duplicate_lifetime(load_setup, monkeypatch, role):
    s = load_setup
    owner, receipt = _completed(s)
    artifact = getattr(receipt, role)
    with monkeypatch.context() as patch:
        patch.setattr(os, 'open', lambda *a, **kw: pytest.fail('public borrow reopened output'))
        fd = owner.public_descriptor(role)
        assert fd == owner._files.public_descriptor(role)
        _readonly(fd)
        duplicate = os.dup(fd)
    try:
        owner.close()
        raw = os.pread(duplicate, artifact.bytes + 1, 0)
        assert len(raw) == artifact.bytes and hashlib.sha256(raw).hexdigest() == artifact.sha256
        _readonly(duplicate)
        assert artifact.path.exists()
        with pytest.raises(OSError): os.fstat(fd)
        with pytest.raises(load.NativeLoadError): owner.public_descriptor(role)
    finally:
        os.close(duplicate)


@pytest.mark.parametrize('role', ['resource_config', 'resource_worker', 'resource_capture_dir',
    'trace', 'journal', '../collector_journal', 'genesis.json', '', None, b'collector_journal'])
def test_completed_load_borrow_has_no_private_or_alias_roles(load_setup, role):
    owner, _ = _completed(load_setup)
    with pytest.raises(load.NativeLoadError, match='^native_load_failed$'): owner.public_descriptor(role)
    assert owner._phase == 'failed'
    with pytest.raises(load.NativeLoadError): owner.public_descriptor('collector_journal')


@pytest.mark.parametrize('layer', ['files', 'load'])
def test_load_borrow_requires_complete_native_capture(load_setup, layer):
    owner = load_setup.create()
    target = owner._files if layer == 'files' else owner
    exception = LoadOutputError if layer == 'files' else load.NativeLoadError
    with pytest.raises(exception): target.public_descriptor('collector_journal')
    assert not load_setup.factory.calls


@pytest.mark.parametrize('role', ['resource_config', 'resource_capture_dir', 'trace', 'journal'])
def test_physical_load_owner_also_rejects_private_files_and_aliases(load_setup, role):
    owner, _ = _completed(load_setup)
    with pytest.raises(LoadOutputError): owner._files.public_descriptor(role)
    assert owner._files._failed


@pytest.mark.parametrize('source', ['genesis', 'load'])
def test_closed_borrowed_number_fails_without_consuming_other_originals(ready_setup, load_setup, source):
    if source == 'genesis':
        owner, role, exception = ready_setup.inputs, 'genesis.json', inputs.ReadinessError
    else:
        owner, _ = _completed(load_setup)
        role, exception = 'collector_journal', load.NativeLoadError
    fd = owner.public_descriptor(role)
    os.close(fd)
    with pytest.raises(exception): owner.public_descriptor(role)
    with pytest.raises(exception): owner.close()
    owner.close()
    with pytest.raises(OSError): os.fstat(fd)


@pytest.mark.parametrize('role', LOAD)
@pytest.mark.parametrize('change', ['replace', 'modify', 'mode', 'hardlink', 'parent', 'stage'])
def test_completed_load_borrow_rechecks_file_and_parent_custody(load_setup, role, change):
    s = load_setup
    owner, _ = _completed(s)
    path = getattr(s.paths, role)
    if change == 'replace': _replace(path)
    elif change == 'modify': path.write_bytes(path.read_bytes() + b' ')
    elif change == 'mode': path.chmod(0o644)
    elif change == 'hardlink': os.link(path, s.c.root / 'second-link')
    elif change == 'parent':
        path.parent.rename(path.parent.with_name('old-evidence'))
        path.parent.mkdir(mode=0o700)
    elif change == 'stage': s.paths.transaction_trace.with_name('trace.json.collecting').write_bytes(b'partial')
    with pytest.raises(load.NativeLoadError): owner.public_descriptor(role)


@pytest.mark.parametrize('timing', ['before', 'during'])
def test_completed_load_borrow_cannot_outlive_original_deadline(load_setup, timing):
    s = load_setup
    owner, _ = _completed(s)
    original = owner.trial_deadline_ns
    if timing == 'before': s.c.clock.now = original
    else: owner._runtime = lambda: setattr(s.c.clock, 'now', original)
    with pytest.raises(load.NativeLoadError): owner.public_descriptor('collector_journal')
    assert owner.trial_deadline_ns == original and owner._phase == 'failed'


@pytest.mark.parametrize('change', ['receipt', 'plan', 'budget', 'runtime', 'reentrant', 'deadline_identity'])
def test_completed_load_borrow_retains_original_authority_guards(load_setup, change):
    s = load_setup
    owner, receipt = _completed(s)
    if change == 'receipt': object.__setattr__(receipt, 'scheduled_requests', 39)
    elif change == 'plan': object.__setattr__(owner._plan, 'max_status_requests', 17)
    elif change == 'budget': object.__setattr__(owner._allocation.run.transaction_trace, 'max_bytes', 1)
    elif change == 'runtime': owner._runtime = lambda: (_ for _ in ()).throw(RuntimeError('PRIVATE TEST'))
    elif change == 'reentrant': owner._runtime = lambda: owner.public_descriptor('transaction_trace')
    elif change == 'deadline_identity': owner._end += 1
    with pytest.raises(load.NativeLoadError, match='^native_load_failed$'):
        owner.public_descriptor('collector_journal')
    assert owner._phase == 'failed'


@pytest.mark.parametrize('source', ['genesis', 'files'])
def test_selected_leaf_change_during_later_original_check_is_rejected(ready_setup, load_setup, monkeypatch, source):
    if source == 'genesis':
        owner, role, exception = ready_setup.inputs, 'genesis-anchors.json', inputs.ReadinessError
        path = ready_setup.directory / role
    else:
        actual, _ = _completed(load_setup)
        owner, role, exception = actual._files, 'transaction_trace', LoadOutputError
        path = load_setup.paths.transaction_trace
    original_validate = owner.validate
    calls = []
    def validate():
        original_validate()
        calls.append(True)
        if len(calls) == 2: _replace(path)
    monkeypatch.setattr(owner, 'validate', validate)
    with pytest.raises(exception): owner.public_descriptor(role)
    assert len(calls) == 2
