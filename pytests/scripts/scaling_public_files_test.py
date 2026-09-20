"""Real-file physical handoff regressions; no native process or release verdict."""
from dataclasses import replace
import fcntl
import hashlib
import os
from pathlib import Path

import pytest

import resource_evidence_budget as budget
import scaling_native_outputs as outputs
import scaling_public_files as public


def allocation(cap=512 * 1024):
    geometry = budget.CaptureGeometry(4, budget.NS, 20 * budget.NS, budget.NS)
    runs = tuple(budget.RunBudget(pair, variant, geometry,
        *(budget.FileBudget(f'p{pair}.{variant}.{role}', cap) for role in budget.RUN_FILE_FIELDS))
        for pair in range(1, 6) for variant in ('one_lane', 'four_lane'))
    whole = budget.admit_experiment(policy=budget.CapturePolicy(1, 1), runs=runs,
        static_files=(budget.StaticFile('source', 1),),
        manifest=budget.FileBudget('manifest', cap), report=budget.FileBudget('report', cap), other_control=())
    return budget.select_run_budget(whole, 1, 'one_lane')


def write(path, raw):
    with path.open('xb') as output:
        os.fchmod(output.fileno(), 0o600)
        output.write(raw)
    return os.open(path, public._READ)


class TrialFiles:
    """Actual NativeOutputs plus read-only stand-ins for the other file owners."""
    def __init__(self, base):
        self.base = base
        self.root = base/'runs'/'pair-01'/'one_lane'
        self.root.mkdir(parents=True, mode=0o700)
        self.budget = allocation()
        self.public = public.RunPublicFiles(self.root, self.budget)
        self.native = outputs.NativeOutputs(self.root/'native', outputs.NativeOutputBudget(*(512 * 1024,) * 6, 3 * 1024 * 1024))
        self.files = {}
        self.expired = False
        self.guard_calls = 0
        for step, roles in outputs._STEPS:
            self.native.begin(step)
            claims = []
            for role in roles:
                raw = (role + ':native-data').encode()
                path = self.native.path(role)
                fd = write(path, raw); os.close(fd)
                claims.append(outputs.PublishedIdentity(role, hashlib.sha256(raw).hexdigest(), len(raw)))
            self.native.complete(tuple(claims))
        for role in ('collector_journal', 'transaction_trace'):
            raw = (role + ':original').encode()
            path = self.root/public._EXISTING[role]
            self.files[role] = (write(path, raw), path, raw)
        self.originals = base/'private'
        self.originals.mkdir(mode=0o700)
        for role, relative in public._GENESIS.items():
            raw = (role + ':genesis-bytes').encode()
            path = self.originals/Path(relative).name
            self.files[role] = (write(path, raw), path, raw)

    def guard(self):
        self.guard_calls += 1
        if self.expired: raise TimeoutError('private-test-deadline')
        self.native.validate()

    def adopt(self, role, guard=None):
        if role.startswith('native_') or role == 'canonical_proof':
            native_role = 'proof' if role == 'canonical_proof' else role.removeprefix('native_')
            artifact = self.native.artifact(native_role)
            self.public.adopt_existing(role, self.native.descriptor(native_role), artifact.sha256,
                artifact.bytes, native_parent=self.native.directory_descriptor(), guard=guard or self.guard)
        else:
            fd, _, raw = self.files[role]
            self.public.adopt_existing(role, fd, hashlib.sha256(raw).hexdigest(), len(raw),
                                       native_parent=None, guard=guard or self.guard)

    def copy(self, role, guard=None):
        fd, _, raw = self.files[role]
        self.public.copy_genesis(role, fd, hashlib.sha256(raw).hexdigest(), len(raw), guard=guard or self.guard)

    def populate(self):
        for role in public._EXISTING: self.adopt(role)
        for role in public._GENESIS: self.copy(role)
        return self.public.seal_sources(guard=self.guard)

    def private_close(self):
        self.native.close()
        for fd, _, _ in self.files.values(): os.close(fd)
        self.files.clear()

    def close(self):
        self.public.close()
        self.private_close()


@pytest.fixture
def trial(tmp_path):
    value = TrialFiles(tmp_path.resolve())
    yield value
    value.close()


def failed(value):
    with pytest.raises(public.PublicFileError, match='^public_file_custody_failed$'):
        value.verify()


def test_original_custody_survives_private_close_and_expired_deadline(trial):
    controls = trial.populate()
    assert len(controls) == 13
    original_calls = trial.guard_calls
    assert all(item.binding.path.startswith('runs/pair-01/one_lane/') for item in controls)
    original_fds = [item.fd for item in trial.public._files.values()]
    for role in public._GENESIS:
        copied = trial.public._files[role]
        original, _, raw = trial.files[role]
        assert os.fstat(original).st_ino != os.fstat(copied.fd).st_ino
        assert os.pread(copied.fd, len(raw), 0) == raw
    trial.private_close(); trial.expired = True
    assert len(trial.public.verify()) == 13
    for item in controls:
        raw = trial.public.read_control(item.binding, max_bytes=item.max_bytes)
        assert hashlib.sha256(raw).hexdigest() == item.binding.sha256
    assert trial.guard_calls == original_calls
    trial.public.publish_summary('raw_run', b'{"samples":[]}', guard=lambda: None)
    trial.public.publish_summary('run_receipt', b'{"physical_only":true}', guard=lambda: None)
    assert len(trial.public.verify()) == 15
    assert len(list(trial.root.rglob('*.publishing'))) == 0
    assert all(os.fstat(fd).st_nlink == 1 for fd in original_fds)
    trial.public.close()
    for fd in original_fds:
        with pytest.raises(OSError): os.fstat(fd)
    assert len(list(trial.root.rglob('*'))) == 17


@pytest.mark.parametrize('role', ['', 'config.toml', 'client.toml', 'private_key', 'trial_log', 'support',
                                  '../genesis.json', True, None])
def test_no_private_or_retired_role_can_be_exported(trial, role):
    fd, _, raw = trial.files['genesis_manifest']
    with pytest.raises(public.PublicFileError):
        trial.public.copy_genesis(role, fd, hashlib.sha256(raw).hexdigest(), len(raw), guard=trial.guard)
    failed(trial.public)
    assert not (trial.root/'genesis').exists()


@pytest.mark.parametrize('method', ['adopt', 'copy', 'seal'])
def test_expired_original_guard_never_admits_outputs(trial, method):
    trial.expired = True
    with pytest.raises(public.PublicFileError):
        if method == 'adopt': trial.adopt('collector_journal')
        elif method == 'copy': trial.copy('genesis_manifest')
        else: trial.public.seal_sources(guard=trial.guard)
    failed(trial.public)
    assert not trial.public._sealed


@pytest.mark.parametrize('when', ['source_read', 'final_seal'])
def test_deadline_crossing_during_copy_or_final_admission_fails(trial, monkeypatch, when):
    if when == 'source_read':
        original = public.os.pread
        def expires(fd, count, offset):
            raw = original(fd, count, offset)
            trial.expired = True
            return raw
        monkeypatch.setattr(public.os, 'pread', expires)
        operation = lambda: trial.copy('genesis_manifest')
    else:
        for role in public._EXISTING: trial.adopt(role)
        for role in public._GENESIS: trial.copy(role)
        def expired():
            trial.guard()
            trial.expired = True
        operation = lambda: trial.public.seal_sources(guard=expired)
    with pytest.raises(public.PublicFileError): operation()
    failed(trial.public)
    assert not trial.public._sealed


@pytest.mark.parametrize('kind', ['same_bytes', 'in_place', 'hardlink', 'symlink', 'mode', 'ancestor', 'unexpected', 'stage'])
def test_changes_after_admission_cannot_be_rebaselined(trial, kind):
    trial.populate()
    path = trial.root/'collector.jsonl'
    if kind == 'same_bytes':
        raw = path.read_bytes(); path.unlink(); fd = write(path, raw); os.close(fd)
    elif kind == 'in_place':
        with path.open('r+b') as output: output.write(b'x')
    elif kind == 'hardlink': os.link(path, trial.base/'alias')
    elif kind == 'symlink':
        path.unlink(); path.symlink_to(trial.files['genesis_manifest'][1])
    elif kind == 'mode': path.chmod(0o640)
    elif kind == 'ancestor':
        trial.root.parent.rename(trial.root.parent.with_name('moved'))
    else:
        extra = trial.root/('collector.jsonl.publishing' if kind == 'stage' else 'extra')
        extra.write_bytes(b'x')
    failed(trial.public)


@pytest.mark.parametrize('phase', ['seal', 'verify', 'read'])
def test_earlier_control_change_during_later_read_fails(trial, monkeypatch, phase):
    if phase == 'seal':
        for role in public._EXISTING: trial.adopt(role)
        for role in public._GENESIS: trial.copy(role)
    else: trial.populate()
    earlier = trial.root/'collector.jsonl'
    later = trial.public._files['genesis_anchors'].fd
    original = public.os.pread
    changed = False
    def mutate(fd, count, offset):
        nonlocal changed
        raw = original(fd, count, offset)
        if fd == later and not changed:
            with earlier.open('r+b') as output: output.write(b'!')
            changed = True
        return raw
    monkeypatch.setattr(public.os, 'pread', mutate)
    with pytest.raises(public.PublicFileError):
        if phase == 'seal': trial.public.seal_sources(guard=trial.guard)
        elif phase == 'verify': trial.public.verify()
        else:
            item = trial.public._files['genesis_anchors'].public
            trial.public.read_control(item.binding, max_bytes=item.max_bytes)
    assert changed
    failed(trial.public)


@pytest.mark.parametrize('kind', ['writable', 'inherited', 'wrong_original', 'wrong_hash', 'wrong_size', 'too_large'])
def test_admission_rejects_unbound_or_unbounded_source(trial, kind):
    fd, path, raw = trial.files['collector_journal']
    digest, size = hashlib.sha256(raw).hexdigest(), len(raw)
    temporary = None
    if kind == 'writable': temporary = fd = os.open(path, os.O_RDWR | os.O_CLOEXEC)
    elif kind == 'inherited': os.set_inheritable(fd, True)
    elif kind == 'wrong_original': fd = trial.files['transaction_trace'][0]
    elif kind == 'wrong_hash': digest = 'a' * 64
    elif kind == 'wrong_size': size += 1
    else: size = 512 * 1024 + 1
    try:
        with pytest.raises(public.PublicFileError):
            trial.public.adopt_existing('collector_journal', fd, digest, size, native_parent=None, guard=trial.guard)
        failed(trial.public)
    finally:
        if temporary is not None: os.close(temporary)


@pytest.mark.parametrize('kind', ['second_adoption', 'second_copy', 'incomplete_seal', 'second_seal', 'late_source',
                                  'early_summary', 'wrong_summary_order', 'second_summary'])
def test_phase_reuse_is_permanently_rejected(trial, kind):
    with pytest.raises(public.PublicFileError):
        if kind == 'second_adoption': trial.adopt('collector_journal'); trial.adopt('collector_journal')
        elif kind == 'second_copy': trial.copy('genesis_manifest'); trial.copy('genesis_manifest')
        elif kind == 'incomplete_seal': trial.public.seal_sources(guard=trial.guard)
        elif kind == 'early_summary': trial.public.publish_summary('raw_run', b'{}', guard=trial.guard)
        else:
            trial.populate()
            if kind == 'second_seal': trial.public.seal_sources(guard=trial.guard)
            elif kind == 'late_source': trial.copy('genesis_manifest')
            elif kind == 'wrong_summary_order': trial.public.publish_summary('run_receipt', b'{}', guard=lambda: None)
            else:
                trial.public.publish_summary('raw_run', b'{}', guard=lambda: None)
                trial.public.publish_summary('raw_run', b'{}', guard=lambda: None)
    failed(trial.public)


@pytest.mark.parametrize('fault', ['short_writes', 'zero_write', 'fsync', 'link_conflict', 'source_mutation'])
def test_copy_publication_checks_actual_writes_and_preserves_conflicting_files(trial, monkeypatch, fault):
    path = trial.root/'genesis'/'genesis.json'
    write_original, fsync_original, link_original = os.pwrite, os.fsync, os.link
    if fault == 'short_writes':
        monkeypatch.setattr(public.os, 'pwrite', lambda fd, raw, offset: write_original(fd, raw[:3], offset))
    elif fault == 'zero_write': monkeypatch.setattr(public.os, 'pwrite', lambda *_: 0)
    elif fault == 'fsync':
        def fail_sync(fd):
            if not os.path.isdir(f'/dev/fd/{fd}'): raise OSError('test-fsync')
            return fsync_original(fd)
        monkeypatch.setattr(public.os, 'fsync', fail_sync)
    elif fault == 'link_conflict':
        def conflict(*args, **kwargs):
            fd = write(path, b'foreign-final'); os.close(fd)
            return link_original(*args, **kwargs)
        monkeypatch.setattr(public.os, 'link', conflict)
    else:
        def mutation(fd, raw, offset):
            with trial.files['genesis_manifest'][1].open('r+b') as output: output.write(b'!')
            return write_original(fd, raw, offset)
        monkeypatch.setattr(public.os, 'pwrite', mutation)
    if fault == 'short_writes':
        trial.copy('genesis_manifest')
        assert path.read_bytes() == trial.files['genesis_manifest'][2]
        assert not path.with_name('genesis.json.publishing').exists()
    else:
        with pytest.raises(public.PublicFileError): trial.copy('genesis_manifest')
        failed(trial.public)
        if fault == 'link_conflict': assert path.read_bytes() == b'foreign-final'


def test_copy_is_streamed_and_enforces_cap_before_reading(trial, monkeypatch):
    role = 'genesis_manifest'
    fd, path, _ = trial.files[role]
    os.close(fd); path.unlink()
    raw = b'g' * (3 * 65536 + 1)
    trial.files[role] = (write(path, raw), path, raw)
    original, counts = os.pread, []
    def capture(fd, count, offset):
        counts.append(count)
        return original(fd, count, offset)
    monkeypatch.setattr(public.os, 'pread', capture)
    trial.copy(role)
    assert counts and max(counts) <= 65536 and len(counts) >= 12


@pytest.mark.parametrize('kind', ['equal_binding', 'cap_small', 'cap_large', 'bool_cap', 'mutated_binding'])
def test_bounded_reads_require_original_binding(trial, kind):
    control = trial.populate()[0]
    binding, cap = control.binding, control.max_bytes
    if kind == 'equal_binding': binding = replace(binding)
    elif kind == 'cap_small': cap = control.bytes - 1
    elif kind == 'cap_large': cap += 1
    elif kind == 'bool_cap': cap = True
    else: object.__setattr__(binding, 'sha256', 'b' * 64)
    with pytest.raises(public.PublicFileError): trial.public.read_control(binding, max_bytes=cap)
    failed(trial.public)


def test_caller_budget_mutation_does_not_retarget_owned_allocation(trial):
    old = trial.public._caps['signed_genesis']
    object.__setattr__(trial.budget.run.signed_genesis, 'max_bytes', 1)
    trial.populate()
    assert trial.public._caps['signed_genesis'] == old
    assert len(trial.public.verify()) == 13


def test_reused_owned_fd_is_not_closed_as_foreign(trial):
    trial.populate()
    owned = trial.public._files['collector_journal'].fd
    foreign_path = trial.base/'foreign'
    foreign = write(foreign_path, b'keep-open')
    os.dup2(foreign, owned, inheritable=False)
    failed(trial.public)
    trial.public.close()
    assert os.pread(owned, 9, 0) == b'keep-open'
    assert os.pread(foreign, 9, 0) == b'keep-open'
    os.close(owned); os.close(foreign)


def test_root_is_admitted_before_producer_files_and_rejects_reuse(tmp_path):
    root = tmp_path.resolve()/'runs'/'pair-01'/'one_lane'
    root.mkdir(parents=True, mode=0o700)
    (root/'old').write_bytes(b'old')
    with pytest.raises(public.PublicFileError): public.RunPublicFiles(root, allocation())
    assert (root/'old').read_bytes() == b'old'


def test_symlinked_ancestor_cannot_be_first_admission(tmp_path):
    root = tmp_path.resolve()
    actual = root/'actual'; actual.mkdir()
    (root/'alias').symlink_to(actual, target_is_directory=True)
    path = root/'alias'/'runs'/'pair-01'/'one_lane'
    path.mkdir(parents=True, mode=0o700)
    with pytest.raises(public.PublicFileError): public.RunPublicFiles(path, allocation())


def capture_genesis_stage_fd(trial, monkeypatch):
    """Observe only the actual stage open, regardless of other owned descriptors."""
    original_open = os.open
    opened = []
    def open_file(path, flags, *args, **kwargs):
        fd = original_open(path, flags, *args, **kwargs)
        if (path == 'genesis.json.publishing'
                and kwargs.get('dir_fd') == trial.public._directories['genesis'][0]):
            opened.append(fd)
        return fd
    monkeypatch.setattr(public.os, 'open', open_file)
    def descriptor():
        assert len(opened) == 1, 'created stage descriptor absent'
        info = os.fstat(opened[0])
        named = (trial.root/'genesis/genesis.json.publishing').stat()
        assert (info.st_dev, info.st_ino) == (named.st_dev, named.st_ino)
        return opened[0]
    return descriptor


def test_stage_descriptor_replacement_is_rejected_before_any_foreign_write(trial, monkeypatch):
    stage_fd = capture_genesis_stage_fd(trial, monkeypatch)
    foreign_path = trial.base/'foreign-write-target'
    foreign = os.open(foreign_path, public._WRITE, 0o600)
    os.write(foreign, b'unchanged-foreign')
    switched = None
    def switch_guard():
        nonlocal switched
        trial.guard()
        if switched is not None or not (trial.root/'genesis/genesis.json.publishing').exists(): return
        switched = stage_fd()
        os.dup2(foreign, switched, inheritable=False)
    try:
        with pytest.raises(public.PublicFileError): trial.copy('genesis_manifest', guard=switch_guard)
        assert switched is not None
        assert os.pread(foreign, 100, 0) == b'unchanged-foreign'
        assert os.pread(switched, 100, 0) == b'unchanged-foreign'
    finally:
        if switched is not None: os.close(switched)
        os.close(foreign)


def test_stage_replacement_during_directory_fsync_is_never_unlinked(trial, monkeypatch):
    original = os.fsync
    stage = trial.root/'genesis/genesis.json.publishing'
    final = trial.root/'genesis/genesis.json'
    changed = False
    def replace_stage(fd):
        nonlocal changed
        if not changed and final.exists() and stage.exists():
            stage.unlink()
            descriptor = write(stage, b'foreign-stage'); os.close(descriptor)
            changed = True
        return original(fd)
    monkeypatch.setattr(public.os, 'fsync', replace_stage)
    with pytest.raises(public.PublicFileError): trial.copy('genesis_manifest')
    assert changed
    assert stage.read_bytes() == b'foreign-stage'
    failed(trial.public)


def test_failed_genesis_directory_creation_does_not_close_reused_fd(trial, monkeypatch):
    original = os.fsync
    foreign = write(trial.base/'foreign-directory-substitute', b'foreign-descriptor')
    switched = None
    def replace_directory(fd):
        nonlocal switched
        if switched is None and 'genesis' in trial.public._directories:
            switched = trial.public._directories['genesis'][0]
            os.dup2(foreign, switched, inheritable=False)
            raise OSError('injected-parent-sync-failure')
        return original(fd)
    monkeypatch.setattr(public.os, 'fsync', replace_directory)
    try:
        with pytest.raises(public.PublicFileError): trial.copy('genesis_manifest')
        assert switched is not None
        assert os.pread(switched, 100, 0) == b'foreign-descriptor'
    finally:
        if switched is not None:
            try: os.close(switched)
            except OSError: pass
        os.close(foreign)


def test_write_may_not_rebaseline_changed_append_flags(trial, monkeypatch):
    original = os.pwrite
    def change_flags(fd, raw, offset):
        result = original(fd, raw, offset)
        fcntl.fcntl(fd, fcntl.F_SETFL, fcntl.fcntl(fd, fcntl.F_GETFL) | os.O_APPEND)
        return result
    monkeypatch.setattr(public.os, 'pwrite', change_flags)
    with pytest.raises(public.PublicFileError): trial.copy('genesis_manifest')
    failed(trial.public)


def test_moved_shared_cursor_cannot_exceed_the_output_reservation(trial, monkeypatch):
    stage_fd = capture_genesis_stage_fd(trial, monkeypatch)
    stage = trial.root/'genesis/genesis.json.publishing'
    moved = False
    def move_cursor():
        nonlocal moved
        trial.guard()
        if moved or not stage.exists(): return
        os.lseek(stage_fd(), trial.public._caps['genesis_manifest'][1] + 65536, os.SEEK_SET)
        moved = True
    trial.copy('genesis_manifest', guard=move_cursor)
    assert moved
    result = trial.root/'genesis/genesis.json'
    assert result.read_bytes() == trial.files['genesis_manifest'][2]
    assert result.stat().st_size < trial.public._caps['genesis_manifest'][1]


def test_scope_accessors_return_original_path_and_separately_owned_budget(trial):
    assert trial.public.directory == trial.root
    value = trial.public.allocation
    assert value == trial.budget and value is not trial.budget
    object.__setattr__(value.run.signed_genesis, 'max_bytes', 1)
    assert trial.public.allocation.run.signed_genesis.max_bytes == 512 * 1024
    trial.populate()
    assert len(trial.public.verify()) == 13


@pytest.mark.parametrize('field', ['directory', 'allocation'])
def test_scope_accessor_rejects_and_poisons_changed_original_parent(trial, field):
    trial.root.rename(trial.root.with_name('moved'))
    trial.root.mkdir(mode=0o700)
    with pytest.raises(public.PublicFileError): getattr(trial.public, field)
    failed(trial.public)
