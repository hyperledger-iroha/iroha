"""Private real-file custody regressions; no native command or proof is executed."""
from dataclasses import replace
import fcntl
import hashlib
import os
from pathlib import Path
import stat
from types import SimpleNamespace

import pytest

import scaling_native_outputs as outputs


def budget(cap=65536):
    return outputs.NativeOutputBudget(*(cap for _ in range(6)), cap * 6)


@pytest.fixture
def owner(tmp_path):
    value = outputs.NativeOutputs(tmp_path.resolve() / 'outputs', budget())
    yield value
    value.close()


def write(path, raw=b'canonical-test-bytes'):
    with path.open('xb') as file:
        os.fchmod(file.fileno(), 0o600)
        file.write(raw)
    return hashlib.sha256(raw).hexdigest(), len(raw)


def stage(owner, role, raw=b'canonical-test-bytes'):
    path = owner.path(role)
    pending = path.with_name(path.name + outputs._STAGES[role])
    digest, length = write(pending, raw)
    owner.validate()
    pending.rename(path)
    owner.validate()
    return outputs.PublishedIdentity(role, digest, length)


def complete(owner, step):
    owner.begin(step)
    roles = dict(outputs._STEPS)[step]
    claims = tuple(stage(owner, role, (role + ':canonical-test').encode()) for role in roles)
    return owner.complete(claims)


def test_complete_fixed_pipeline_retains_all_originals_until_close(owner):
    original = {}
    for step, roles in outputs._STEPS:
        artifacts = complete(owner, step)
        assert tuple(row.role for row in artifacts) == roles
        for row in artifacts:
            fd = owner.descriptor(row.role)
            original[row.role] = fd
            assert os.pread(fd, row.bytes, 0) == (row.role + ':canonical-test').encode()
            assert row.max_bytes == 65536 and row.path == owner.directory / (row.role + '.nrt')
            assert fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE == os.O_RDONLY
            assert stat.S_IMODE(os.fstat(fd).st_mode) == 0o600
    result = owner.finish()
    assert tuple(row.role for row in result) == outputs._ROLES
    assert tuple(path.name for path in sorted(owner.directory.iterdir())) == tuple(sorted(role + '.nrt' for role in outputs._ROLES))
    owner.close()
    owner.close()
    for fd in original.values():
        with pytest.raises(OSError): os.fstat(fd)
    assert all(row.path.exists() for row in result)
    with pytest.raises(outputs.NativeOutputError): owner.finish()


def test_completed_parent_descriptor_is_original_and_borrowed(owner):
    for step, _ in outputs._STEPS: complete(owner, step)
    original = owner.directory_descriptor()
    assert original == owner._root
    assert not os.get_inheritable(original)
    duplicate = os.dup(original)
    try:
        assert os.fstat(duplicate) == os.fstat(original)
        owner.close()
        assert len(os.listdir(duplicate)) == 6
    finally:
        os.close(duplicate)


def test_parent_descriptor_requires_complete_outputs_and_rejects_namespace_drift(owner):
    complete(owner, 'collection')
    with pytest.raises(outputs.NativeOutputError): owner.directory_descriptor()
    with pytest.raises(outputs.NativeOutputError): owner.validate()


def test_completed_parent_descriptor_rejects_original_directory_replacement(owner):
    for step, _ in outputs._STEPS: complete(owner, step)
    original = owner.directory
    original.rename(original.with_name('moved'))
    original.mkdir(mode=0o700)
    with pytest.raises(outputs.NativeOutputError): owner.directory_descriptor()


@pytest.mark.parametrize('step', ['facts', 'prepare', 'export', '', 'collection ', True, None])
def test_wrong_first_step_is_terminal(owner, step):
    with pytest.raises(outputs.NativeOutputError): owner.begin(step)
    with pytest.raises(outputs.NativeOutputError): owner.begin('collection')


def test_next_command_cannot_begin_before_complete_group(owner):
    owner.begin('collection')
    with pytest.raises(outputs.NativeOutputError): owner.begin('facts')
    with pytest.raises(outputs.NativeOutputError): owner.validate()


@pytest.mark.parametrize('operation', ['path', 'allocation', 'artifact', 'descriptor', 'finish'])
def test_invalid_lookup_or_incomplete_finish_permanently_closes_admission(owner, operation):
    with pytest.raises(outputs.NativeOutputError):
        if operation == 'finish': owner.finish()
        else: getattr(owner, operation)('undeclared')
    with pytest.raises(outputs.NativeOutputError): owner.begin('collection')


@pytest.mark.parametrize('field,value', [('finality', True), ('queries', 0), ('facts', -1),
    ('request', 268435457), ('bundle', 1.0), ('proof', None), ('total', 1), ('total', 2147483649)])
def test_allocations_are_admitted_before_directory_creation(tmp_path, field, value):
    path = tmp_path.resolve() / 'outputs'
    with pytest.raises(outputs.NativeOutputError): outputs.NativeOutputs(path, replace(budget(), **{field: value}))
    assert not path.exists()


def test_budget_values_are_owned_not_borrowed(tmp_path):
    original = budget()
    with outputs.NativeOutputs(tmp_path.resolve() / 'outputs', original) as owner:
        object.__setattr__(original, 'proof', 1)
        assert owner._caps['proof'] == 65536
        assert owner.allocation('proof') == 65536
        assert complete(owner, 'collection')[0].max_bytes == 65536


@pytest.mark.parametrize('kind', ['directory', 'file', 'symlink'])
def test_existing_output_namespace_is_never_reused(tmp_path, kind):
    path = tmp_path.resolve() / 'outputs'
    if kind == 'directory': path.mkdir()
    elif kind == 'file': write(path)
    else: path.symlink_to(tmp_path.resolve(), target_is_directory=True)
    with pytest.raises(outputs.NativeOutputError): outputs.NativeOutputs(path, budget())
    assert path.exists()


def test_lexical_symlink_ancestor_is_rejected(tmp_path):
    root = tmp_path.resolve()
    (root / 'alias').symlink_to(root, target_is_directory=True)
    with pytest.raises(outputs.NativeOutputError): outputs.NativeOutputs(root / 'alias' / 'outputs', budget())
    assert not (root / 'outputs').exists()


@pytest.mark.parametrize('name', ['unlisted', 'facts.nrt', 'proof.nrt.publishing',
    'finality.nrt.publishing', 'queries.nrt.collecting.more'])
def test_only_current_commands_exact_stage_and_final_names_may_appear(owner, name):
    owner.begin('collection')
    write(owner.directory / name)
    with pytest.raises(outputs.NativeOutputError): owner.validate()
    assert (owner.directory / name).exists()


def test_no_native_file_may_appear_before_its_step(owner):
    write(owner.directory / 'finality.nrt')
    with pytest.raises(outputs.NativeOutputError): owner.begin('collection')


@pytest.mark.parametrize('kind', ['directory', 'symlink', 'fifo', 'hardlink', 'mode', 'oversize'])
def test_pending_output_type_owner_and_byte_bounds(owner, kind):
    owner.begin('collection')
    path = owner.path('finality')
    if kind == 'directory': path.mkdir()
    elif kind == 'symlink': path.symlink_to(owner.directory, target_is_directory=True)
    elif kind == 'fifo': os.mkfifo(path, mode=0o600)
    elif kind == 'hardlink':
        outside = owner.directory.parent / 'outside'
        write(outside)
        os.link(outside, path)
    else:
        write(path, b'x' * (65537 if kind == 'oversize' else 1))
        if kind == 'mode': path.chmod(0o644)
    with pytest.raises(outputs.NativeOutputError): owner.validate()


def test_native_stage_and_destination_cannot_coexist(owner):
    owner.begin('collection')
    write(owner.directory / 'finality.nrt')
    write(owner.directory / 'finality.nrt.collecting')
    with pytest.raises(outputs.NativeOutputError): owner.validate()


@pytest.mark.parametrize('failure', ['empty', 'prefix', 'reorder', 'extra', 'digest', 'length', 'bool', 'missing', 'stage'])
def test_reply_and_output_group_must_be_complete(owner, failure):
    owner.begin('collection')
    claims = [stage(owner, role) for role in ('finality', 'queries')]
    if failure == 'empty': claims = []
    elif failure == 'prefix': claims.pop()
    elif failure == 'reorder': claims.reverse()
    elif failure == 'extra': claims.append(claims[0])
    elif failure == 'digest': claims[-1] = replace(claims[-1], sha256='0' * 64)
    elif failure == 'length': claims[-1] = replace(claims[-1], bytes=1)
    elif failure == 'bool': claims[-1] = replace(claims[-1], bytes=True)
    elif failure == 'missing': (owner.directory / 'queries.nrt').unlink()
    else: (owner.directory / 'queries.nrt').rename(owner.directory / 'queries.nrt.collecting')
    with pytest.raises(outputs.NativeOutputError): owner.complete(tuple(claims))
    with pytest.raises(outputs.NativeOutputError): owner.artifact('finality')
    # Earlier successful opens remain owned for close(), never public results.
    fds = [item.fd for item in owner._files.values()]
    owner.close()
    for fd in fds:
        with pytest.raises(OSError): os.fstat(fd)


def test_later_file_failure_cannot_publish_earlier_output(owner):
    owner.begin('collection')
    first, second = (stage(owner, role) for role in ('finality', 'queries'))
    with pytest.raises(outputs.NativeOutputError): owner.complete((first, replace(second, sha256='0' * 64)))
    assert set(owner._files) == {'finality', 'queries'}
    with pytest.raises(outputs.NativeOutputError): owner.descriptor('finality')


@pytest.mark.parametrize('change', ['replace', 'content', 'chmod', 'hardlink', 'descriptor', 'fd_flags'])
def test_sealed_original_changes_are_detected_before_next_step(owner, change):
    complete(owner, 'collection')
    artifact = owner.artifact('finality')
    fd = owner.descriptor('finality')
    if change == 'replace':
        raw = artifact.path.read_bytes()
        artifact.path.unlink()
        write(artifact.path, raw)
    elif change == 'content': artifact.path.write_bytes(b'changed')
    elif change == 'chmod': artifact.path.chmod(0o400)
    elif change == 'hardlink': os.link(artifact.path, owner.directory.parent / 'alias')
    elif change == 'descriptor': os.dup2(owner.descriptor('queries'), fd)
    else: fcntl.fcntl(fd, fcntl.F_SETFL, fcntl.fcntl(fd, fcntl.F_GETFL) ^ os.O_NONBLOCK)
    with pytest.raises(outputs.NativeOutputError): owner.begin('facts')
    assert owner._failed


def test_root_replacement_does_not_adopt_same_named_directory(owner):
    original = owner.directory
    original.rename(original.with_name('moved'))
    original.mkdir(mode=0o700)
    with pytest.raises(outputs.NativeOutputError): owner.validate()
    assert original.exists() and original.with_name('moved').exists()


def test_capture_rechecks_earlier_output_when_later_hash_is_read(owner, monkeypatch):
    owner.begin('collection')
    claims = tuple(stage(owner, role) for role in ('finality', 'queries'))
    read = os.pread
    changed = []
    def replace_earlier(fd, count, offset):
        if 'queries' in owner._files and fd == owner._files['queries'].fd and not changed:
            changed.append(True)
            path = owner.directory / 'finality.nrt'
            raw = path.read_bytes()
            path.unlink()
            write(path, raw)
        return read(fd, count, offset)
    monkeypatch.setattr(os, 'pread', replace_earlier)
    with pytest.raises(outputs.NativeOutputError): owner.complete(claims)
    assert changed and owner._failed


def test_capture_parent_metadata_bracket_rejects_transient_namespace_change(owner, monkeypatch):
    owner.begin('collection')
    claims = tuple(stage(owner, role) for role in ('finality', 'queries'))
    read, changed = os.pread, []
    def alter_parent(fd, count, offset):
        if 'queries' in owner._files and fd == owner._files['queries'].fd and not changed:
            changed.append(True)
            transient = owner.directory / 'transient'
            write(transient)
            transient.unlink()
        return read(fd, count, offset)
    monkeypatch.setattr(os, 'pread', alter_parent)
    with pytest.raises(outputs.NativeOutputError, match='namespace_changed_during_capture'): owner.complete(claims)
    assert changed


def test_each_hash_read_is_bounded_and_failure_retains_original_descriptor(tmp_path, monkeypatch):
    with outputs.NativeOutputs(tmp_path.resolve() / 'outputs', budget(1024 * 1024)) as owner:
        owner.begin('collection')
        claims = tuple(stage(owner, role, b'x' * 800000) for role in ('finality', 'queries'))
        read, requests = os.pread, []
        def bounded(fd, count, offset):
            requests.append(count)
            return read(fd, count, offset)
        monkeypatch.setattr(os, 'pread', bounded)
        artifacts = owner.complete(claims)
        assert max(requests) == 65536 and len(requests) > 20
        assert all(row.bytes == 800000 for row in artifacts)


def test_active_atomic_stage_rename_retries_only_changed_namespace(owner, monkeypatch):
    owner.begin('collection')
    pending = owner.directory / 'finality.nrt.collecting'
    write(pending)
    actual_scan, calls = owner._scan, []
    def rename_once():
        calls.append(True)
        if len(calls) == 1:
            pending.rename(owner.directory / 'finality.nrt')
            raise FileNotFoundError('old stage vanished')
        return actual_scan()
    monkeypatch.setattr(owner, '_scan', rename_once)
    owner.validate()
    assert len(calls) == 2 and not owner._failed


def test_atomic_rename_between_iterator_entries_may_observe_both_names(owner, monkeypatch):
    owner.begin('collection')
    pending = owner.directory / 'finality.nrt.collecting'
    final = owner.directory / 'finality.nrt'
    write(pending)
    actual, calls = os.scandir, []
    class DuringRename:
        def __enter__(self): return self.entries()
        def __exit__(self, *args): pass
        def entries(self):
            yield SimpleNamespace(name=pending.name)
            pending.rename(final)
            yield SimpleNamespace(name=final.name)
    def scan(fd):
        calls.append(fd)
        return DuringRename() if len(calls) == 1 else actual(fd)
    monkeypatch.setattr(os, 'scandir', scan)
    owner.validate()
    assert len(calls) == 2 and final.is_file() and not pending.exists()


def test_rename_retry_cannot_hide_changed_previously_sealed_output(owner, monkeypatch):
    complete(owner, 'collection')
    original = owner.directory / 'finality.nrt'
    owner.begin('facts')
    pending = owner.directory / 'facts.nrt.publishing'
    write(pending)
    def scan():
        pending.rename(owner.directory / 'facts.nrt')
        original.write_bytes(b'changed original')
        raise outputs._ActiveRenameObservation('native_output_stage_and_final_coexist')
    monkeypatch.setattr(owner, '_scan', scan)
    with pytest.raises(outputs.NativeOutputError): owner.validate()
    assert owner._failed


def test_missing_file_without_namespace_change_is_terminal(owner, monkeypatch):
    owner.begin('collection')
    def missing(): raise FileNotFoundError('PRIVATE')
    monkeypatch.setattr(owner, '_scan', missing)
    with pytest.raises(outputs.NativeOutputError, match='^native_output_namespace_changed$'): owner.validate()


def test_repeated_active_namespace_mutation_is_bounded(owner, monkeypatch):
    owner.begin('collection')
    calls = []
    def change():
        calls.append(True)
        temporary = owner.directory / 'finality.nrt.collecting'
        write(temporary)
        temporary.unlink()
        return set()
    monkeypatch.setattr(owner, '_scan', change)
    with pytest.raises(outputs.NativeOutputError, match='namespace_unstable'): owner.validate()
    assert len(calls) == 8


def test_reentrant_validation_cannot_be_caught_and_promoted(owner, monkeypatch):
    actual = owner._scan
    def reenter():
        with pytest.raises(outputs.NativeOutputError): owner.validate()
        return actual()
    monkeypatch.setattr(owner, '_scan', reenter)
    with pytest.raises(outputs.NativeOutputError): owner.validate()


def test_close_does_not_close_a_foreign_descriptor_reusing_an_old_number(owner):
    complete(owner, 'collection')
    fd = owner.descriptor('finality')
    foreign = owner.directory.parent / 'foreign'
    write(foreign)
    other = os.open(foreign, os.O_RDONLY)
    try:
        os.dup2(other, fd)
        owner.close()
        assert os.pread(fd, 1, 0) == b'c'
    finally:
        os.close(other)
        os.close(fd)
