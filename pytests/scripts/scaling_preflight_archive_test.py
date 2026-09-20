"""Actual file and inert-data tests for the source-bound preflight archive."""
from dataclasses import FrozenInstanceError
from pathlib import Path
import copy
import fcntl
import json
import os
import shutil

import pytest
import scaling_preflight_archive as m
import scaling_preflight_archive_fixture as fixture

ROOT = Path(__file__).resolve().parents[2]
IDENTITY = dict(head_commit='1' * 40, head_tree='2' * 40,
                workspace_source_manifest_sha256='3' * 64)


@pytest.fixture(scope='module')
def original(tmp_path_factory):
    root = tmp_path_factory.mktemp('preflight-archive-source') / 'archive'
    binding = fixture.build_archive(ROOT, root, IDENTITY)
    return root, binding


@pytest.fixture
def specimen(original, tmp_path):
    root, binding = original
    destination = tmp_path / 'archive'
    shutil.copytree(root, destination)
    return destination, copy.deepcopy(binding)


def inspect(root, binding, **overrides):
    context = dict(source_root=ROOT, candidate_identity=IDENTITY,
                   invocation_sha256='e' * 64, collector_started_ns=1001, timeout_seconds=600)
    context.update(overrides)
    return m.inspect_preflight_archive(root, binding, **context)


def index(root):
    return json.loads((root / 'index.json').read_bytes())


def change_unit(root, binding, role, change, ordinal=0):
    value = index(root)
    name = m._unit_name(ordinal, role)
    document = json.loads((root / name).read_bytes())
    change(document)
    value['units'][ordinal][role] = fixture.write_member(root, name, m.canonical(document))
    fixture.rebind(root, binding, value)


def test_complete_archive_capture_is_immutable_relocation_safe_data(original, tmp_path):
    root, binding = original
    checked = inspect(root, binding)
    assert not hasattr(checked, 'passed') and not hasattr(checked, 'process')
    with pytest.raises(FrozenInstanceError):
        checked.index_json = b'forged'
    context = dict(source_root=ROOT, candidate_identity=IDENTITY,
                   invocation_sha256='e' * 64, collector_started_ns=1001, timeout_seconds=600)
    files, directories = m.capture_preflight_archive(root, binding, **context)
    assert len(files) == 4 * (len(m.PHASE_COUNTS) + len(m.REQUIRED_PYTEST_SUITES)) + 2
    assert directories == ({'relative_path': '', 'mode': '0700'},)
    assert all(set(row) == {'relative_path', 'sha256', 'size_bytes', 'mode', 'max_bytes'}
               and row['mode'] == '0400' for row in files)
    assert m.archive_census(files) == binding['inventory']
    relocated = fixture.copy_selected_source(ROOT, tmp_path / 'relocated-source')
    assert inspect(root, binding, source_root=relocated) == checked
    assert not Path(index(root)['command_context']['repository_root']).exists()
    assert (root / 'unit-000.result.json').read_bytes() != m.canonical(
        json.loads((root / 'unit-000.result.json').read_bytes()))


@pytest.mark.parametrize('kind', ['missing', 'unknown', 'boolean', 'float', 'zero', 'deadline',
                                  'timeout', 'hash', 'mode', 'files', 'bytes', 'old_schema'])
def test_compact_binding_exact_schema_and_bounds(original, kind):
    value = copy.deepcopy(original[1])
    if kind == 'missing': del value['scope']
    elif kind == 'unknown': value['success'] = True
    elif kind == 'boolean': value['scope']['original_started_ns'] = True
    elif kind == 'float': value['scope']['completed_ns'] = 1000.0
    elif kind == 'zero': value['index']['size_bytes'] = 0
    elif kind == 'deadline': value['scope']['deadline_ns'] += 1
    elif kind == 'timeout': value['scope']['timeout_seconds'] = 599
    elif kind == 'hash': value['inventory']['sha256'] = 'A' * 64
    elif kind == 'mode': value['index']['mode'] = '0600'
    elif kind == 'files': value['inventory']['files'] -= 4
    elif kind == 'bytes': value['inventory']['bytes'] = m._total_cap() + 1
    elif kind == 'old_schema': value['archive_id'] = 'release-scaling.preflight.v0'
    with pytest.raises(m.ScalingPreflightError): m.validate_binding(value)


@pytest.mark.parametrize('kind', ['candidate', 'invocation', 'timeout', 'collector', 'source_inventory'])
def test_required_external_context_cannot_be_replaced_by_archive(original, tmp_path, kind):
    root, binding = original
    context = {}
    if kind == 'candidate': context['candidate_identity'] = dict(IDENTITY, head_tree='4' * 40)
    elif kind == 'invocation': context['invocation_sha256'] = 'f' * 64
    elif kind == 'timeout': context['timeout_seconds'] = 601
    elif kind == 'collector': context['collector_started_ns'] = 999
    else:
        source = fixture.copy_selected_source(ROOT, tmp_path / 'source')
        path = source / m.INVENTORY_PATH
        path.write_bytes(path.read_bytes() + b' ')
        context['source_root'] = source
    with pytest.raises(m.ScalingPreflightError, match='archive is invalid'):
        inspect(root, binding, **context)
    for api in (m.inspect_preflight_archive, m.capture_preflight_archive):
        with pytest.raises(TypeError): api(root, binding)


@pytest.mark.parametrize('kind', ['order', 'missing', 'duplicate', 'unknown', 'candidate', 'invocation',
                                  'scope', 'completion', 'selection', 'context', 'boolean_index'])
def test_rehashed_index_still_checks_semantic_joins(specimen, kind):
    root, binding = specimen
    value = index(root)
    if kind == 'order': value['units'][0], value['units'][1] = value['units'][1], value['units'][0]
    elif kind == 'missing': value['units'].pop()
    elif kind == 'duplicate': value['units'][1] = copy.deepcopy(value['units'][0])
    elif kind == 'unknown': value['untrusted_success'] = True
    elif kind == 'candidate': value['candidate']['head_commit'] = '4' * 40
    elif kind == 'invocation': value['invocation_sha256'] = 'f' * 64
    elif kind == 'scope': value['scope']['deadline_ns'] += 1
    elif kind == 'completion': value['scope']['verification_completed_ns'] = 1001
    elif kind == 'selection': value['selection']['sha256'] = '0' * 64
    elif kind == 'context': value['command_context']['blake3']['bundle_root'] = '/unselected/bundle'
    else: value['units'][0]['index'] = False
    fixture.rebind(root, binding, value)
    with pytest.raises(m.ScalingPreflightError): inspect(root, binding)


@pytest.mark.parametrize('kind', ['argv', 'argv_hash', 'environment', 'cwd', 'pid', 'descriptor',
                                  'violation', 'returncode', 'deadline', 'overlap', 'completion', 'stream', 'unknown'])
def test_rehashed_full_commands_cannot_change_original_fixed_roles(specimen, kind):
    root, binding = specimen
    def change(value):
        if kind == 'argv':
            value['argv'][4] = '/historical/other-driver.py'
            value['argv_sha256'] = fixture.sha(m.canonical(value['argv']))
        elif kind == 'argv_hash': value['argv_sha256'] = '0' * 64
        elif kind == 'environment': value['environment_sha256'] = '0' * 64
        elif kind == 'cwd': value['cwd'] = '/historical/other-root'
        elif kind == 'pid': value['pid'] = True
        elif kind == 'descriptor': value['descriptors'] = [3]
        elif kind == 'violation': value['violations'] = ['output']
        elif kind == 'returncode': value['returncode'] = 1
        elif kind == 'deadline': value['deadline_ns'] += 1
        elif kind == 'overlap': value['started_ns'] = 100
        elif kind == 'completion': value['completed_ns'] = 1001
        elif kind == 'stream': value['stdout']['sha256'] = '0' * 64
        else: value['trusted'] = True
    change_unit(root, binding, 'command', change, ordinal=1 if kind == 'overlap' else 0)
    with pytest.raises(m.ScalingPreflightError): inspect(root, binding)


@pytest.mark.parametrize('kind', ['failure', 'skip', 'empty', 'isolation', 'source', 'copy', 'attempt',
                                  'bootstrap', 'pytest_nodes', 'pytest_inventory'])
def test_rehashed_results_preserve_every_existing_required_assertion(specimen, kind):
    root, binding = specimen
    def change(value):
        if kind == 'failure': value['passed'] = False
        elif kind == 'skip': value['skipped'] = 1
        elif kind == 'empty': value['node_ids'] = []
        elif kind == 'isolation': value['no_site'] = False
        elif kind == 'source': value['inputs_after'] = {}
        elif kind == 'copy': value['copied_sources_after'] = {}
        elif kind == 'attempt': value['forbidden_process_attempts'] = ['subprocess.Popen']
        elif kind == 'bootstrap': value['actual_private_blake3_import'] = False
        elif kind == 'pytest_nodes': value['collected_node_sha256s'] = []
        else: value['inventory_sha256'] = '0' * 64
    change_unit(root, binding, 'result', change, ordinal=len(m.PHASE_COUNTS) if kind.startswith('pytest') else 0)
    with pytest.raises(m.ScalingPreflightError): inspect(root, binding)


@pytest.mark.parametrize('kind', ['missing', 'extra', 'symlink', 'hardlink', 'writable', 'directory'])
def test_actual_archive_namespace_and_metadata(specimen, tmp_path, kind):
    root, binding = specimen
    path = root / 'unit-000.stdout'
    if kind == 'missing': path.unlink()
    elif kind == 'extra': (root / 'unselected.txt').write_bytes(b'no')
    elif kind == 'symlink':
        target = tmp_path / 'foreign'; path.rename(target); path.symlink_to(target)
    elif kind == 'hardlink': os.link(path, tmp_path / 'foreign')
    elif kind == 'writable': path.chmod(0o600)
    else: root.chmod(0o755)
    with pytest.raises(m.ScalingPreflightError): inspect(root, binding)


def test_source_hash_is_checked_even_when_archive_inventory_itself_matches(original, tmp_path):
    root, binding = original
    source = fixture.copy_selected_source(ROOT, tmp_path / 'source')
    path = source / m.PHASE_DRIVER
    path.write_bytes(path.read_bytes() + b'\n# changed after selection\n')
    with pytest.raises(m.ScalingPreflightError): inspect(root, binding, source_root=source)


@pytest.mark.parametrize('raw', [b'{"a":1,"a":2}', b'[' * 13 + b'0' + b']' * 13,
                               b'{"a":NaN}', b'\xff', b'', b'{"a":'])
def test_bounded_json_rejects_duplicates_depth_nonfinite_and_malformed(raw):
    with pytest.raises(m.ScalingPreflightError): m.bounded_json(raw, 1000)


def test_before_spawn_envelope_and_role_limits(original):
    context = index(original[0])['command_context']
    for ordinal, (kind, name, _) in enumerate(m.ordered_units(json.loads((original[0] / 'inventory.json').read_bytes()))):
        argv = m.unit_argv(ordinal, kind, name, context)
        assert argv[1:4] == ('-I', '-B', '-S')
        assert argv[argv.index('--work-root') + 1].endswith('/unit-%03d' % ordinal)
        m.validate_command_inputs(argv, context['repository_root'])
    for argv, cwd in [(('x' * m.MAX_COMMAND_BYTES,), '/a'), (('x\0',), '/a'), (('x',), '/a/../b'),
                      (['x'], '/a'), (('"' * (m.MAX_COMMAND_BYTES // 2),), '/a')]:
        with pytest.raises(m.ScalingPreflightError): m.validate_command_inputs(argv, cwd)


@pytest.mark.parametrize('kind', ['command', 'index', 'combined'])
def test_census_caps_are_independent_of_total_archive_cap(original, kind):
    rows = list(fixture.census(original[0]))
    if kind == 'combined':
        for row in rows:
            if row['relative_path'] in ('unit-000.stdout', 'unit-000.stderr'):
                row['size_bytes'] = m.MAX_OUTPUT_BYTES // 2 + 1
    else:
        name, cap = ('unit-000.command.json', m.MAX_COMMAND_BYTES) if kind == 'command' else ('index.json', m.MAX_INDEX_BYTES)
        next(row for row in rows if row['relative_path'] == name)['size_bytes'] = cap + 1
    with pytest.raises(m.ScalingPreflightError): m.archive_census(rows)


def test_sparse_oversize_rejected_without_reading(specimen, monkeypatch):
    root, binding = specimen
    path = root / 'index.json'; path.chmod(0o600)
    with path.open('wb') as stream: stream.truncate(m.MAX_INDEX_BYTES + 1)
    path.chmod(0o400)
    original = os.pread
    calls = []
    def bounded(fd, count, offset):
        assert os.fstat(fd).st_size <= m.MAX_SOURCE_BYTES
        calls.append(count)
        return original(fd, count, offset)
    monkeypatch.setattr(os, 'pread', bounded)
    with pytest.raises(m.ScalingPreflightError): inspect(root, binding)
    assert calls and max(calls) <= 65536


def test_partial_pread_preserves_exact_bytes_and_cleanup(original, monkeypatch):
    original_pread = os.pread
    monkeypatch.setattr(os, 'pread', lambda fd, count, offset: original_pread(fd, min(count, 777), offset))
    assert inspect(*original).binding_json == m.canonical(original[1])


@pytest.mark.parametrize('kind', ['foreign', 'writable', 'inheritable', 'nonblocking'])
def test_read_error_never_closes_reused_or_drifted_descriptor(tmp_path, monkeypatch, kind):
    root = tmp_path / 'source'; root.mkdir(mode=0o700)
    path = root / 'file'; path.write_bytes(b'original'); path.chmod(0o600)
    foreign = root / 'foreign'; foreign.write_bytes(b'foreign')
    reader = m._Reader(root, archive=False)
    saved = os.pread
    replaced = []
    def fail(fd, count, offset):
        if not replaced:
            if kind in ('foreign', 'writable'):
                other = os.open(foreign if kind == 'foreign' else path, os.O_RDWR | os.O_CLOEXEC)
                os.dup2(other, fd, inheritable=False); os.close(other)
            elif kind == 'inheritable': os.set_inheritable(fd, True)
            else: fcntl.fcntl(fd, fcntl.F_SETFL, fcntl.fcntl(fd, fcntl.F_GETFL) & ~os.O_NONBLOCK)
            replaced.append(fd)
            raise OSError('private secret must not escape through public inspection')
        return saved(fd, count, offset)
    monkeypatch.setattr(os, 'pread', fail)
    try:
        with pytest.raises(OSError): reader.read('file', 100)
        assert os.fstat(replaced[0])
    finally:
        reader.close()
        if replaced: os.close(replaced[0])


@pytest.mark.parametrize('kind', ['foreign', 'inheritable', 'nonblocking'])
def test_retained_directory_drift_is_rejected_and_foreign_slot_preserved(tmp_path, kind):
    root = tmp_path / 'source'; root.mkdir(mode=0o700)
    other = tmp_path / 'other'; other.mkdir(mode=0o700)
    reader = m._Reader(root, archive=False)
    slot = reader.fd
    if kind == 'foreign':
        descriptor = os.open(other, m._DIRECTORY)
        os.dup2(descriptor, slot, inheritable=False); os.close(descriptor)
    elif kind == 'inheritable': os.set_inheritable(slot, True)
    else: fcntl.fcntl(slot, fcntl.F_SETFL, fcntl.fcntl(slot, fcntl.F_GETFL) & ~os.O_NONBLOCK)
    try:
        with pytest.raises(m.ScalingPreflightError): reader.check()
        reader.close()
        assert os.fstat(slot)
    finally: os.close(slot)


def test_root_renamed_and_replaced_after_capture_is_rejected(tmp_path):
    root = tmp_path / 'source'; root.mkdir(mode=0o700)
    reader = m._Reader(root, archive=False)
    root.rename(tmp_path / 'old'); root.mkdir(mode=0o700)
    try:
        with pytest.raises(m.ScalingPreflightError): reader.verify()
    finally: reader.close()


@pytest.mark.parametrize('kind', ['unchanged', 'foreign', 'writable', 'inheritable', 'nonblocking'])
def test_acquisition_pin_failure_cleanup_respects_original_slot(tmp_path, monkeypatch, kind):
    path = tmp_path / 'selected'; path.write_bytes(b'original'); path.chmod(0o600)
    foreign = tmp_path / 'foreign'; foreign.write_bytes(b'foreign')
    original_pin = m._pin
    seen = []
    def fail(fd):
        if not seen:
            seen.append(fd)
            if kind in ('foreign', 'writable'):
                other = os.open(foreign if kind == 'foreign' else path, os.O_RDWR | os.O_CLOEXEC)
                os.dup2(other, fd, inheritable=False); os.close(other)
            elif kind == 'inheritable': os.set_inheritable(fd, True)
            elif kind == 'nonblocking': fcntl.fcntl(fd, fcntl.F_SETFL, 0)
            raise OSError('pin unavailable')
        return original_pin(fd)
    monkeypatch.setattr(m, '_pin', fail)
    with pytest.raises(OSError): m._acquire(path, m._READ, path.stat())
    if kind == 'unchanged':
        with pytest.raises(OSError): os.fstat(seen[0])
    else:
        try: assert os.fstat(seen[0])
        finally: os.close(seen[0])


def test_constructor_error_closes_original_ancestors_preserves_reused_root(tmp_path, monkeypatch):
    root = tmp_path / 'source'; root.mkdir(mode=0o700)
    foreign = tmp_path / 'foreign'; foreign.mkdir(mode=0o700)
    closed, preserved = [], []
    old_check, old_close = m._Reader.check, os.close
    def fail(self):
        preserved.append(self.fd)
        fd = os.open(foreign, m._DIRECTORY)
        os.dup2(fd, self.fd, inheritable=False); old_close(fd)
        old_check(self)
    def close(fd): closed.append(fd); old_close(fd)
    monkeypatch.setattr(m._Reader, 'check', fail)
    monkeypatch.setattr(os, 'close', close)
    with pytest.raises(m.ScalingPreflightError): m._Reader(root, archive=False)
    try:
        assert preserved[0] not in closed and closed
        assert os.fstat(preserved[0])
    finally: old_close(preserved[0])


def test_directory_iteration_cap_counts_yielded_entries_not_only_unique_names(monkeypatch):
    # A changing directory can repeat entries. Bound iteration independently of
    # the final exact unique-name census, without retaining an unbounded list.
    class Entries:
        count = 0
        def __enter__(self): return self
        def __exit__(self, *error): return None
        def __iter__(self): return self
        def __next__(self):
            self.count += 1
            assert self.count <= 3
            return type('Entry', (), {'name': 'repeated'})()
    entries = Entries()
    monkeypatch.setattr(os, 'scandir', lambda descriptor: entries)
    with pytest.raises(m.ScalingPreflightError): m._names(42, 2)
    assert entries.count == 3


@pytest.mark.parametrize('kind', ['foreign', 'inheritable', 'nonblocking'])
def test_nested_directory_replacement_during_read_preserves_foreign_slot(tmp_path, monkeypatch, kind):
    root = tmp_path / 'source'; root.mkdir(mode=0o700)
    nested = root / 'a' / 'b'; nested.mkdir(mode=0o700, parents=True)
    (nested / 'file').write_bytes(b'original')
    foreign = tmp_path / 'foreign'; foreign.mkdir(mode=0o700)
    reader = m._Reader(root, archive=False)
    original_parent, original_pread = reader._parent, os.pread
    held, replaced = [], []
    def parent(name):
        result = original_parent(name)
        held[:] = result[2]
        return result
    def read(fd, count, offset):
        if not replaced:
            slot = held[-1][0]
            if kind == 'foreign':
                other = os.open(foreign, m._DIRECTORY)
                os.dup2(other, slot, inheritable=False); os.close(other)
            elif kind == 'inheritable': os.set_inheritable(slot, True)
            else: fcntl.fcntl(slot, fcntl.F_SETFL, fcntl.fcntl(slot, fcntl.F_GETFL) & ~os.O_NONBLOCK)
            replaced.append(slot)
        return original_pread(fd, count, offset)
    monkeypatch.setattr(reader, '_parent', parent)
    monkeypatch.setattr(os, 'pread', read)
    try:
        with pytest.raises((m.ScalingPreflightError, OSError)): reader.read('a/b/file', 100)
        assert os.fstat(replaced[0])
        with pytest.raises(OSError): os.fstat(held[0][0])
    finally:
        reader.close()
        if replaced: os.close(replaced[0])


def test_public_archive_error_suppresses_private_read_details(original, monkeypatch):
    import traceback
    def failed(*arguments): raise OSError('private-path-or-seed-must-not-appear')
    monkeypatch.setattr(os, 'pread', failed)
    with pytest.raises(m.ScalingPreflightError) as failure:
        inspect(*original)
    assert str(failure.value) == 'fixed scaling preflight archive is invalid'
    assert failure.value.__suppress_context__ and failure.value.__cause__ is None
    assert 'private-path-or-seed-must-not-appear' not in ''.join(traceback.format_exception(failure.value))


@pytest.mark.parametrize('archive', [False, True])
def test_unrelated_ancestor_sibling_change_preserves_original_handle_ownership(tmp_path, archive):
    parent = tmp_path / 'ancestor'; parent.mkdir(mode=0o700)
    root = parent / 'selected'; root.mkdir(mode=0o700)
    path = root / 'file'; path.write_bytes(b'original'); path.chmod(0o400)
    reader = m._Reader(root, archive=archive)
    handles = tuple(reader.handles)
    before = parent.stat()
    try:
        assert reader.read('file', 8) == b'original'
        (parent / 'unrelated-sibling').mkdir(mode=0o700)
        after = parent.stat()
        assert after.st_ino == before.st_ino and after.st_nlink != before.st_nlink
        reader.check()
        assert reader.read('file', 8) == b'original'
        reader.verify({'file'})
    finally:
        reader.close()
        leaked = []
        for fd, pin in handles:
            try: current = os.fstat(fd)
            except OSError: continue
            leaked.append(fd)
            if (current.st_dev, current.st_ino) == pin[0][:2]: os.close(fd)
        assert leaked == []


def test_owned_archive_root_link_count_change_still_rejects_without_leaking(tmp_path):
    root = tmp_path / 'archive'; root.mkdir(mode=0o700)
    reader = m._Reader(root, archive=True)
    handles = tuple(reader.handles)
    (root / 'unselected-directory').mkdir(mode=0o700)
    try:
        with pytest.raises(m.ScalingPreflightError): reader.check()
    finally:
        reader.close()
        for fd, _ in handles:
            with pytest.raises(OSError): os.fstat(fd)


@pytest.mark.parametrize('archive', [False, True])
def test_semantic_directory_mode_drift_rejects_without_leaking_original_handles(tmp_path, archive):
    root = tmp_path / 'selected'; root.mkdir(mode=0o700)
    reader = m._Reader(root, archive=archive)
    handles = tuple(reader.handles)
    root.chmod(0o500)
    try:
        with pytest.raises(m.ScalingPreflightError): reader.check()
    finally:
        reader.close()
        for fd, _ in handles:
            with pytest.raises(OSError): os.fstat(fd)


def test_semantic_file_mode_drift_rejects_and_closes_original_read_handle(tmp_path, monkeypatch):
    root = tmp_path / 'selected'; root.mkdir(mode=0o700)
    path = root / 'file'; path.write_bytes(b'original'); path.chmod(0o400)
    reader = m._Reader(root, archive=False)
    original, changed = os.pread, []
    def read(fd, count, offset):
        if not changed:
            os.fchmod(fd, 0o600)
            changed.append(fd)
        return original(fd, count, offset)
    monkeypatch.setattr(os, 'pread', read)
    try:
        with pytest.raises(m.ScalingPreflightError): reader.read('file', 8)
        assert len(changed) == 1
        with pytest.raises(OSError): os.fstat(changed[0])
    finally: reader.close()
