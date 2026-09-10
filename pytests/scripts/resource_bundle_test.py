"""Synthetic physical bundles exercise exact typed counts, identity and allocation."""
from dataclasses import replace
import hashlib
import os
from pathlib import Path
import socket
import sys
import tempfile
from unittest.mock import patch

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts' / 'nexus'))
from resource_evidence_budget import (
    BudgetError, CaptureGeometry, CapturePolicy, FileBudget, RunBudget, StaticFile, admit_experiment,
)
from resource_bundle import BudgetedBundle, BundleError, ControlBinding


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def budget():
    geometry = CaptureGeometry(4, 2_000_000, 40_000_000, 2_000_000)
    runs = tuple(RunBudget(pair, variant, geometry,
                          *(FileBudget(f'p{pair}.{variant}.{role}', 64)
                            for role in ('journal', 'trace', 'proof', 'log', 'raw')), ())
                 for pair in range(1, 6) for variant in ('one_lane', 'four_lane'))
    return admit_experiment(policy=CapturePolicy(128, 256), runs=runs,
                            static_files=(StaticFile('source', 1),),
                            manifest=FileBudget('manifest', 64),
                            report=FileBudget('report', 64), other_control=())


def bindings(admitted, reported=False):
    dynamic = (*admitted.control_budgets,
               *(item for run in admitted.runs for item in run.files))
    return (ControlBinding('source', 'pinned/source', digest(b'x')),
            *(ControlBinding(item.label, f'controls/{item.label}', digest(b'x'))
              for item in dynamic if reported or item.label != 'report'))


@pytest.fixture
def valid(tmp_path):
    root = (tmp_path / 'bundle').resolve()
    root.mkdir()
    admitted = budget()
    controls = bindings(admitted)
    for item in controls:
        path = root / item.path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b'x')
    for run in admitted.runs:
        path = root / 'resources' / f'pair-{run.pair_index:02}' / run.variant
        path.mkdir(parents=True, mode=0o700)
        for sequence in range(admitted.geometry.sample_count + 1):
            prefix = f'{"preflight" if sequence == 0 else "sample"}-{sequence:010}'
            names = [f'{prefix}.json']
            names += [f'{prefix}-peer-{peer:04}-{role}.body'
                      for peer in range(4) for role in ('status', 'metrics')]
            for name in names:
                member = path / name
                member.write_bytes(b'x')
                member.chmod(0o600)
    return root, admitted, controls


def member(root, name='sample-0000000001-peer-0000-status.body'):
    return root / 'resources/pair-01/one_lane' / name


def test_complete_fanout_is_scanned_twice_without_relaxing_control_count(valid):
    root, admitted, controls = valid
    with BudgetedBundle(root, admitted, controls, reported=False) as capture:
        first = capture.snapshot
        assert first.resource_files == 2070 > 256
        assert first.control_files == 52
        assert first.total_bytes == 2122
        assert first.resource_bytes == 2070
        assert len(first.census_sha256) == 64
        assert capture.verify() == first
        assert first.total_bytes < admitted.total_bytes <= 2 * 1024 ** 3
    with pytest.raises(BundleError, match='bundle_closed'):
        capture.verify()
    capture.close()


@pytest.mark.parametrize('field', ['resource_member_count', 'bytes_per_capture', 'resource_bytes',
                                   'total_bytes', 'control_file_count'])
def test_forged_computed_budget_fails_before_open(valid, field):
    root, admitted, controls = valid
    with patch('resource_bundle.os.open', side_effect=AssertionError('must not open')):
        with pytest.raises(BudgetError, match='admitted_experiment_mismatch'):
            BudgetedBundle(root, replace(admitted, **{field: getattr(admitted, field) - 1}),
                           controls, reported=False)


@pytest.mark.parametrize('value', ['../x', '/x', 'a//x', 'a/./x', 'a/../x', 'resources/x',
                                   'a b', 'x\n', 'x/' + 'y' * 129, '/'.join(['a'] * 9)])
def test_control_path_admission(value):
    with pytest.raises(BundleError, match='path_invalid'):
        ControlBinding('a', value, digest(b'x'))


@pytest.mark.parametrize('change', ['missing', 'duplicate_label', 'duplicate_path', 'wrong_label', 'list'])
def test_exact_control_mapping_before_io(valid, change):
    root, admitted, controls = valid
    if change == 'missing': controls = controls[:-1]
    elif change == 'duplicate_label': controls = (*controls[:-1], controls[0])
    elif change == 'duplicate_path': controls = (*controls[:-1], replace(controls[-1], path=controls[0].path))
    elif change == 'wrong_label': controls = (*controls[:-1], replace(controls[-1], label='unknown'))
    else: controls = list(controls)
    with patch('resource_bundle.os.open', side_effect=AssertionError('must not open')):
        with pytest.raises(BundleError):
            BudgetedBundle(root, admitted, controls, reported=False)


@pytest.mark.parametrize('name', ['sample-0000000000.json', 'preflight-0000000001.json',
                                  'sample-0000000023.json', 'sample-0000000001-peer-0004-status.body',
                                  'sample-0000000001-peer-0000-extra.body', 'sample-1.json'])
def test_invalid_capture_name_cannot_replace_required_member(valid, name):
    root, admitted, controls = valid
    member(root, 'sample-0000000001.json').rename(member(root, name))
    with pytest.raises(BundleError):
        BudgetedBundle(root, admitted, controls, reported=False)


@pytest.mark.parametrize('change', ['missing', 'extra', 'extra_dir', 'empty_dir', 'extra_control'])
def test_closed_inventory(valid, change):
    root, admitted, controls = valid
    if change == 'missing': member(root).unlink()
    elif change == 'extra': member(root, 'sample-0000000001-peer-0004-status.body').write_bytes(b'x')
    elif change == 'extra_dir': member(root, 'unexpected').mkdir()
    elif change == 'empty_dir': (root / 'unexpected').mkdir()
    else: (root / 'controls/unallocated').write_bytes(b'x')
    with pytest.raises(BundleError):
        BudgetedBundle(root, admitted, controls, reported=False)


@pytest.mark.parametrize('role,cap', [('status', 128), ('metrics', 256)])
def test_policy_exact_body_cap_and_one_byte_over(valid, role, cap):
    root, admitted, controls = valid
    path = member(root, f'sample-0000000001-peer-0000-{role}.body')
    path.write_bytes(b'x' * cap)
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        assert captured.verify().resource_bytes == 2069 + cap
    path.write_bytes(b'x' * (cap + 1))
    with pytest.raises(BundleError, match='file_allocation_exceeded'):
        BudgetedBundle(root, admitted, controls, reported=False)


@pytest.mark.parametrize('kind', ['symlink', 'hardlink', 'fifo', 'socket', 'directory', 'mode', 'empty'])
def test_special_or_unsafe_resource_file_fails_without_blocking(valid, kind):
    root, admitted, controls = valid
    path = member(root)
    held = None
    path.unlink()
    try:
        if kind == 'symlink': path.symlink_to(member(root, 'sample-0000000001.json'))
        elif kind == 'hardlink': os.link(member(root, 'sample-0000000001.json'), path)
        elif kind == 'fifo': os.mkfifo(path)
        elif kind == 'socket':
            # macOS Unix-domain pathname length is bounded independently.
            held = socket.socket(socket.AF_UNIX)
            with tempfile.TemporaryDirectory(prefix='gb-', dir='/private/tmp') as short_root:
                short = Path(short_root) / 's'
                held.bind(str(short))
                short.rename(path)
        elif kind == 'directory': path.mkdir()
        else:
            path.write_bytes(b'' if kind == 'empty' else b'x')
            path.chmod(0o644 if kind == 'mode' else 0o600)
        with pytest.raises(BundleError):
            BudgetedBundle(root, admitted, controls, reported=False)
    finally:
        if held is not None: held.close()


@pytest.mark.parametrize('change', ['digest', 'static_size', 'dynamic_size', 'capture_dir_mode'])
def test_control_identity_and_allocation_checks(valid, change):
    root, admitted, controls = valid
    if change == 'digest': (root / controls[1].path).write_bytes(b'y')
    elif change == 'static_size': (root / controls[0].path).write_bytes(b'xx')
    elif change == 'dynamic_size': (root / controls[1].path).write_bytes(b'x' * 65)
    else: member(root).parent.chmod(0o755)
    with pytest.raises(BundleError):
        BudgetedBundle(root, admitted, controls, reported=False)


@pytest.mark.parametrize('change', ['contents', 'inode', 'mode', 'root', 'ancestor', 'directory'])
def test_changes_during_semantic_replay_fail_final_census(valid, change):
    root, admitted, controls = valid
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        if change == 'contents': member(root).write_bytes(b'y')
        elif change == 'inode':
            member(root).unlink()
            member(root).write_bytes(b'x')
            member(root).chmod(0o600)
        elif change == 'mode': member(root).chmod(0o644)
        elif change == 'directory':
            directory = member(root).parent
            directory.rename(directory.with_name('retired'))
            directory.mkdir(mode=0o700)
        else:
            directory = root if change == 'root' else root.parent
            directory.rename(directory.with_name(directory.name + '-retired'))
            directory.mkdir()
        with pytest.raises((BundleError, OSError)):
            captured.verify()


def test_report_allocation_is_reserved_but_stage_requires_exact_presence(valid):
    root, admitted, controls = valid
    with pytest.raises(BundleError, match='control_mapping_invalid'):
        BudgetedBundle(root, admitted, controls, reported=True)
    report = ControlBinding('report', 'controls/report', digest(b'x'))
    (root / report.path).write_bytes(b'x')
    with pytest.raises(BundleError):
        BudgetedBundle(root, admitted, controls, reported=False)
    with BudgetedBundle(root, admitted, (*controls, report), reported=True) as captured:
        assert captured.verify().control_files == 53


def test_parent_symlink_is_rejected(valid):
    root, admitted, controls = valid
    linked = root.parent / 'linked'
    linked.symlink_to(root, target_is_directory=True)
    with pytest.raises(OSError):
        BudgetedBundle(linked, admitted, controls, reported=False)


def test_failure_releases_all_scanner_descriptors(valid):
    root, admitted, controls = valid
    member(root).unlink()
    original_open, original_close = os.open, os.close
    opened, closed = [], []
    def opening(*args, **kwargs):
        fd = original_open(*args, **kwargs)
        opened.append(fd)
        return fd
    def closing(fd):
        closed.append(fd)
        original_close(fd)
    with patch('resource_bundle.os.open', opening), patch('resource_bundle.os.close', closing):
        with pytest.raises(BundleError):
            BudgetedBundle(root, admitted, controls, reported=False)
    assert sorted(opened) == sorted(closed)


@pytest.mark.parametrize('kind', ['same_inode_write', 'replace_inode'])
def test_actual_read_boundary_rechecks_named_and_held_file(valid, kind):
    root, admitted, controls = valid
    target = member(root)
    inode = target.stat().st_ino
    original = os.pread
    changed = []
    def reading(fd, count, offset):
        result = original(fd, count, offset)
        if os.fstat(fd).st_ino == inode and not changed:
            changed.append(True)
            if kind == 'replace_inode': target.unlink()
            target.write_bytes(b'y')
            target.chmod(0o600)
        return result
    with patch('resource_bundle.os.pread', reading):
        with pytest.raises(BundleError, match='file_changed'):
            BudgetedBundle(root, admitted, controls, reported=False)
    assert changed == [True]


def test_full_control_budget_remains_independent_of_raw_fanout(valid):
    root, old, controls = valid
    extras = tuple(FileBudget(f'extra{i}', 1) for i in range(203))
    admitted = admit_experiment(policy=old.policy, runs=old.runs, static_files=old.static_files,
                                manifest=old.control_budgets[0], report=old.control_budgets[1],
                                other_control=extras)
    additional = tuple(ControlBinding(item.label, f'controls/{item.label}', digest(b'x'))
                       for item in extras)
    report = ControlBinding('report', 'controls/report', digest(b'x'))
    for item in (*additional, report):
        (root / item.path).write_bytes(b'x')
    with BudgetedBundle(root, admitted, (*controls, *additional, report), reported=True) as captured:
        assert captured.verify().control_files == 256
        assert captured.snapshot.resource_files == 2070
        assert admitted.control_file_count == 256


@pytest.mark.parametrize('change', ['relative', 'dotdot', 'wrong_stage', 'invalid_digest', 'bad_budget_controls'])
def test_additional_exact_input_bounds_before_io(valid, change):
    root, admitted, controls = valid
    reported = False
    if change == 'relative': root = Path('relative')
    elif change == 'dotdot': root = root / '..' / root.name
    elif change == 'wrong_stage': reported = 0
    elif change == 'invalid_digest':
        altered = replace(controls[0])
        object.__setattr__(altered, 'sha256', 'invalid')
        controls = (altered, *controls[1:])
    else: admitted = replace(admitted, control_budgets=())
    with patch('resource_bundle.os.open', side_effect=AssertionError('must not open')):
        with pytest.raises((BundleError, BudgetError)):
            BudgetedBundle(root, admitted, controls, reported=reported)


@pytest.mark.parametrize('field,value', [('status_body_bytes', 0), ('status_body_bytes', True),
                                        ('metrics_body_bytes', -1)])
def test_mutated_nested_policy_is_readmitted_before_io(valid, field, value):
    root, admitted, controls = valid
    altered = replace(admitted.policy)
    object.__setattr__(altered, field, value)
    admitted = replace(admitted, policy=altered)
    with patch('resource_bundle.os.open', side_effect=AssertionError('must not open')):
        with pytest.raises(BudgetError):
            BudgetedBundle(root, admitted, controls, reported=False)


@pytest.mark.parametrize('failure', [MemoryError, KeyboardInterrupt, RuntimeError])
@pytest.mark.parametrize('point', ['scan', 'first_fstat', 'child_fstat'])
def test_every_acquired_descriptor_closed_on_arbitrary_constructor_failure(valid, failure, point):
    root, admitted, controls = valid
    original_open, original_close, original_fstat = os.open, os.close, os.fstat
    opened, closed, metadata_reads = [], [], []
    def opening(*args, **kwargs):
        fd = original_open(*args, **kwargs)
        opened.append(fd)
        return fd
    def closing(fd):
        closed.append(fd)
        original_close(fd)
    def reading(fd):
        metadata_reads.append(fd)
        if ((point == 'first_fstat' and len(metadata_reads) == 1)
                or (point == 'child_fstat' and len(metadata_reads) == 2)):
            raise failure('injected owner metadata failure')
        return original_fstat(fd)
    def scanning(_):
        raise failure('injected scan failure')
    with (patch('resource_bundle.os.open', opening), patch('resource_bundle.os.close', closing),
          patch('resource_bundle.os.fstat', reading), patch.object(BudgetedBundle, '_scan', scanning)):
        with pytest.raises(failure):
            BudgetedBundle(root, admitted, controls, reported=False)
    assert len(opened) > 0
    assert sorted(opened) == sorted(closed)


@pytest.mark.parametrize('path', [Path('/' + '/'.join(['x'] * 65)), Path('/' + 'x' * 4096)])
def test_ancestor_descriptor_and_root_byte_bounds_precede_open(valid, path):
    _, admitted, controls = valid
    with patch('resource_bundle.os.open', side_effect=AssertionError('must not open')):
        with pytest.raises(BundleError, match='root_path_invalid'):
            BudgetedBundle(path, admitted, controls, reported=False)


@pytest.mark.parametrize('change', ['valid_budget_replacement', 'nested_policy', 'control_digest',
                                   'control_path', 'stage', 'stage_type', 'oversized_control',
                                   'root', 'root_invalid'])
def test_admitted_scope_cannot_change_while_raw_files_remain_exact(valid, change):
    root, admitted, controls = valid
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        if change == 'valid_budget_replacement':
            captured.budget = admit_experiment(policy=CapturePolicy(129, 256), runs=admitted.runs,
                static_files=admitted.static_files, manifest=admitted.control_budgets[0],
                report=admitted.control_budgets[1], other_control=())
        elif change == 'nested_policy':
            object.__setattr__(captured.budget.policy, 'status_body_bytes', 129)
        elif change == 'control_digest':
            object.__setattr__(controls[0], 'sha256', digest(b'y'))
        elif change == 'control_path':
            object.__setattr__(controls[0], 'path', 'pinned/renamed')
        elif change == 'oversized_control':
            object.__setattr__(controls[0], 'path', 'x' * 1025)
        elif change == 'root':
            captured.root = root.parent
        elif change == 'root_invalid':
            captured.root = Path('relative')
        else:
            captured._reported = True if change == 'stage' else 0
        with patch('resource_bundle.os.fstat', side_effect=AssertionError('scope checked before filesystem')):
            with pytest.raises(BundleError, match='bundle_scope_changed'):
                captured.verify()


import stat


def test_read_control_returns_exact_bytes_for_every_admitted_binding_and_keeps_final_census(valid):
    root, admitted, controls = valid
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        initial = captured.snapshot
        for control in controls:
            raw = captured.read_control(control, max_bytes=1)
            assert type(raw) is bytes and raw == b'x'
        assert len(captured._control_read_admission[0]) == 52
        assert len(captured._control_read_admission[1]) == 3
        assert captured.verify() == initial
    with pytest.raises(BundleError, match='bundle_closed'):
        captured.read_control(controls[0], max_bytes=1)


@pytest.mark.parametrize('kind', ['equal', 'changed_path', 'changed_hash', 'changed_label',
                                 'raw_path', 'wrong_type', 'oversized', 'bool', 'negative', 'float'])
def test_read_control_rejects_unbound_or_invalid_requests_and_permanently_poisons(valid, kind):
    root, admitted, controls = valid
    binding, limit = controls[1], 1
    if kind == 'equal': binding = replace(binding)
    elif kind == 'changed_path': binding = replace(binding, path='controls/unknown')
    elif kind == 'changed_hash': binding = replace(binding, sha256=digest(b'y'))
    elif kind == 'changed_label': binding = replace(binding, label='unknown')
    elif kind == 'raw_path':
        binding = replace(binding)
        object.__setattr__(binding, 'path', 'resources/pair-01/one_lane/preflight-0000000000.json')
    elif kind == 'wrong_type': binding = object()
    elif kind == 'oversized': limit = 65
    elif kind == 'bool': limit = True
    elif kind == 'negative': limit = -1
    elif kind == 'float': limit = 1.0
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        with patch('resource_bundle.os.open', side_effect=AssertionError('no dependent open')):
            with pytest.raises(BundleError):
                captured.read_control(binding, max_bytes=limit)
        with pytest.raises(BundleError, match='control_read_failed'):
            captured.read_control(controls[1], max_bytes=1)
        with pytest.raises(BundleError, match='control_read_failed'):
            captured.verify()


def test_control_semantic_cap_precedes_open_allocation_and_actual_multichunk_read_is_bounded(valid):
    root, old, controls = valid
    size = 2 * 65536 + 1
    raw = b'z' * size
    manifest = old.control_budgets[0]
    admitted = admit_experiment(policy=old.policy, runs=old.runs, static_files=old.static_files,
                                manifest=FileBudget(manifest.label, size),
                                report=old.control_budgets[1], other_control=())
    selected = next(c for c in controls if c.label == manifest.label)
    bound = replace(selected, sha256=digest(raw))
    controls = tuple(bound if c is selected else c for c in controls)
    (root / bound.path).write_bytes(raw)
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        with (patch('resource_bundle.os.open', side_effect=AssertionError('file cap first')),
              patch('resource_bundle.os.pread', side_effect=AssertionError('no byte read'))):
            with pytest.raises(BundleError, match='control_read_cap_exceeded'):
                captured.read_control(bound, max_bytes=size - 1)
    calls = []
    original = os.pread
    def reading(fd, count, offset):
        calls.append((count, offset))
        return original(fd, count, offset)
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        with patch('resource_bundle.os.pread', reading):
            assert captured.read_control(bound, max_bytes=size) == raw
        assert calls == [(65536, 0), (65536, 65536), (1, 131072)]
        assert captured.verify().control_bytes == 51 + size


def test_read_empty_exact_static_control_allows_only_zero_admitted_cap(valid):
    root, old, controls = valid
    admitted = admit_experiment(policy=old.policy, runs=old.runs,
                                static_files=(StaticFile('source', 0),),
                                manifest=old.control_budgets[0], report=old.control_budgets[1],
                                other_control=())
    empty = replace(controls[0], sha256=digest(b''))
    controls = (empty, *controls[1:])
    (root / empty.path).write_bytes(b'')
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        with patch('resource_bundle.os.pread', side_effect=AssertionError('empty read is exact')):
            assert captured.read_control(empty, max_bytes=0) == b''
        assert captured.verify().control_bytes == 51
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        with pytest.raises(BundleError, match='control_read_cap_invalid'):
            captured.read_control(empty, max_bytes=1)


@pytest.mark.parametrize('path', ['resources_extra/control', 'a/b/c/d/e/f/g/control', 'root-control'])
def test_read_control_walks_only_the_admitted_namespace_including_depth_boundary(valid, path):
    root, admitted, controls = valid
    original = controls[0]
    selected = replace(original, path=path)
    old = root / original.path
    old.unlink()
    old.parent.rmdir()
    target = root / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(b'x')
    controls = (selected, *controls[1:])
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        assert captured.read_control(selected, max_bytes=1) == b'x'
        assert captured.verify().control_files == 52


@pytest.mark.parametrize('change', ['symlink', 'hardlink', 'fifo', 'directory', 'inode',
                                   'contents', 'oversize', 'mode', 'missing'])
def test_read_control_rejects_post_admission_file_changes_before_any_byte_read(valid, change):
    root, admitted, controls = valid
    selected = controls[1]
    path = root / selected.path
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        if change in ['symlink', 'hardlink', 'fifo', 'directory', 'inode', 'missing']:
            path.unlink()
        if change == 'symlink': path.symlink_to(root / controls[2].path)
        elif change == 'hardlink': os.link(root / controls[2].path, path)
        elif change == 'fifo': os.mkfifo(path)
        elif change == 'directory': path.mkdir()
        elif change in ['inode', 'contents']: path.write_bytes(b'x' if change == 'inode' else b'y')
        elif change == 'oversize':
            with path.open('r+b') as stream:
                stream.truncate(1024 * 1024 * 1024)
        elif change == 'mode': path.chmod(0o600 if path.stat().st_mode & 0o777 != 0o600 else 0o644)
        with patch('resource_bundle.os.pread', side_effect=AssertionError('must not read replacement')):
            with pytest.raises(BundleError):
                captured.read_control(selected, max_bytes=64)
        with pytest.raises(BundleError, match='control_read_failed'):
            captured.verify()


@pytest.mark.parametrize('kind', ['fifo', 'symlink', 'regular'])
def test_read_control_post_metadata_replacement_uses_nonblocking_nofollow_retained_open(valid, kind):
    root, admitted, controls = valid
    selected = controls[1]
    path = root / selected.path
    original_open = os.open
    race, actual_flags, peer = [], [], []
    def opening(name, flags, *args, **kwargs):
        if name == path.name and kwargs.get('dir_fd') is not None and not race:
            race.append(True)
            path.unlink()
            if kind == 'fifo':
                os.mkfifo(path)
                # Independent writer prevents a regressed blocking read-open from hanging.
                peer.append(original_open(path, os.O_RDWR | os.O_NONBLOCK | os.O_CLOEXEC))
            elif kind == 'symlink': path.symlink_to(root / controls[2].path)
            else: path.write_bytes(b'x')
            actual_flags.append(flags)
        return original_open(name, flags, *args, **kwargs)
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        try:
            with (patch('resource_bundle.os.open', opening),
                  patch('resource_bundle.os.pread', side_effect=AssertionError('replacement bytes forbidden'))):
                with pytest.raises(BundleError):
                    captured.read_control(selected, max_bytes=1)
        finally:
            for fd in peer: os.close(fd)
        assert race == [True]
        assert len(actual_flags) == 1
        required = os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
        assert actual_flags[0] & required == required
        with pytest.raises(BundleError, match='control_read_failed'):
            captured.verify()


@pytest.mark.parametrize('kind', ['symlink', 'directory'])
def test_read_control_directory_replacement_between_metadata_and_open_never_reaches_file(valid, kind):
    root, admitted, controls = valid
    selected = controls[1]
    parent = (root / selected.path).parent
    original_open = os.open
    raced, flags_seen = [], []
    def opening(name, flags, *args, **kwargs):
        if name == parent.name and not raced:
            raced.append(True)
            retired = parent.with_name('retired')
            parent.rename(retired)
            if kind == 'symlink': parent.symlink_to(retired, target_is_directory=True)
            else:
                parent.mkdir()
                (parent / Path(selected.path).name).write_bytes(b'x')
            flags_seen.append(flags)
        return original_open(name, flags, *args, **kwargs)
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        with (patch('resource_bundle.os.open', opening),
              patch('resource_bundle.os.pread', side_effect=AssertionError('unbound directory'))):
            with pytest.raises(BundleError):
                captured.read_control(selected, max_bytes=1)
        assert raced == [True]
        required = os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
        assert flags_seen[0] & required == required


@pytest.mark.parametrize('change', ['same_inode_write', 'replace_same_bytes', 'grow', 'chmod',
                                   'parent', 'root_scope', 'binding_scope', 'policy_scope'])
def test_read_control_rechecks_held_named_directory_and_scope_after_actual_pread(valid, change):
    root, admitted, controls = valid
    selected = controls[1]
    path = root / selected.path
    original_pread = os.pread
    changed = []
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        def reading(fd, count, offset):
            raw = original_pread(fd, count, offset)
            if not changed:
                changed.append(True)
                if change == 'same_inode_write': path.write_bytes(b'y')
                elif change == 'replace_same_bytes':
                    path.unlink()
                    path.write_bytes(b'x')
                elif change == 'grow':
                    with path.open('r+b') as stream: stream.truncate(1024 * 1024 * 1024)
                elif change == 'chmod': path.chmod(0o600 if stat.S_IMODE(path.stat().st_mode) != 0o600 else 0o644)
                elif change == 'parent':
                    path.parent.rename(path.parent.with_name('retired'))
                    path.parent.mkdir()
                elif change == 'root_scope': captured.root = root.parent
                elif change == 'binding_scope': object.__setattr__(selected, 'sha256', digest(b'y'))
                else: object.__setattr__(captured.budget.policy, 'status_body_bytes', 129)
            return raw
        with patch('resource_bundle.os.pread', reading):
            with pytest.raises(BundleError):
                captured.read_control(selected, max_bytes=1)
        assert changed == [True]
        with pytest.raises(BundleError, match='control_read_failed'):
            captured.verify()


@pytest.mark.parametrize('failure', [MemoryError, KeyboardInterrupt, RuntimeError, OSError])
@pytest.mark.parametrize('point', ['directory_fstat', 'file_fstat', 'pread'])
def test_read_control_closes_each_acquired_descriptor_and_poison_survives_caught_failure(valid, failure, point):
    root, admitted, controls = valid
    selected = controls[1]
    original_open, original_close, original_fstat, original_pread = os.open, os.close, os.fstat, os.pread
    opened, closed, raised = [], [], []
    def opening(*args, **kwargs):
        fd = original_open(*args, **kwargs)
        opened.append(fd)
        return fd
    def closing(fd):
        closed.append(fd)
        return original_close(fd)
    def metadata(fd):
        info = original_fstat(fd)
        if fd in opened and not raised and ((point == 'directory_fstat' and stat.S_ISDIR(info.st_mode))
                or (point == 'file_fstat' and stat.S_ISREG(info.st_mode))):
            raised.append(True)
            raise failure('secret-path-material')
        return info
    def reading(*args):
        if point == 'pread':
            raised.append(True)
            raise failure('secret-path-material')
        return original_pread(*args)
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        with (patch('resource_bundle.os.open', opening), patch('resource_bundle.os.close', closing),
              patch('resource_bundle.os.fstat', metadata), patch('resource_bundle.os.pread', reading)):
            with pytest.raises(BundleError if failure is OSError else failure) as caught:
                captured.read_control(selected, max_bytes=1)
        assert raised == [True]
        assert opened and sorted(opened) == sorted(closed)
        if failure is OSError:
            assert str(caught.value) == 'control_read_io'
        with pytest.raises(BundleError, match='control_read_failed'):
            captured.verify()


@pytest.mark.parametrize('return_bytes', [b'', b'xx', b'y'])
def test_read_control_requires_complete_bounded_authenticated_returned_bytes(valid, return_bytes):
    root, admitted, controls = valid
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        with patch('resource_bundle.os.pread', return_value=return_bytes):
            with pytest.raises(BundleError):
                captured.read_control(controls[1], max_bytes=1)
        with pytest.raises(BundleError, match='control_read_failed'):
            captured.verify()


def test_successful_control_read_does_not_replace_required_final_census(valid):
    root, admitted, controls = valid
    with BudgetedBundle(root, admitted, controls, reported=False) as captured:
        assert captured.read_control(controls[1], max_bytes=1) == b'x'
        member(root).write_bytes(b'y')
        with pytest.raises(BundleError):
            captured.verify()
